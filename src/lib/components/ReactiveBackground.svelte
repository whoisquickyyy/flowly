<script lang="ts">
  import { onDestroy } from 'svelte';
  import { audioEngine } from '../audioEngine';
  import type { CoverPalette } from '../colorExtract';
  import * as api from '../api';
  import type { BackgroundMode, LyricsLine, Track } from '../types';

  interface Props {
    mode: BackgroundMode;
    track: Track | null;
    palette: CoverPalette;
    isPlaying: boolean;
    positionSec: number;
    proxyPort: number;
  }

  let { mode, track, palette, isPlaying, positionSec, proxyPort }: Props = $props();

  let canvasEl: HTMLCanvasElement | undefined = $state();
  let rafId = 0;
  let mouseX = 0;
  let mouseY = 0;

  // Для «Живой типографики» (всплывающие слова в тайминг с разными шрифтами и размерами)
  interface KineticWord {
    id: number;
    text: string;
    x: number;
    y: number;
    wPct: number;
    hPct: number;
    size: number;
    font: string;
    italic: boolean;
    upper: boolean;
    rot: number;
    color: string;
  }

  let lyricLines = $state<LyricsLine[]>([]);
  let lyricsTrackId = $state<string | null>(null);
  let kineticWords = $state<KineticWord[]>([]);
  let lastTriggeredLineIdx = -1;
  let wordSeq = 1;
  let wordTimers: ReturnType<typeof setTimeout>[] = [];

  const ALL_FONTS = [
    '"SF Pro Display", -apple-system, "Segoe UI", "Arial", sans-serif',
    '"Inter", "SF Pro Display", "Segoe UI", "Arial", sans-serif',
    '"Arial", "Helvetica Neue", "Segoe UI", sans-serif',
    '"Bahnschrift", "Roboto", "Segoe UI", "Arial", sans-serif',
    '"Manrope", "Segoe UI", "Arial", sans-serif',
    '"IBM Plex Sans", "Segoe UI", "Arial", sans-serif',
  ];

  function pickGenreFonts(genre?: string | null): string[] {
    const g = (genre ?? '').toLowerCase();
    if (/rap|hip|trap|drill|phonk|cloud/.test(g)) {
      return [
        '"Bahnschrift", "Inter", "Arial", sans-serif',
        '"Arial", "Helvetica Neue", "Segoe UI", sans-serif',
        '"SF Pro Display", -apple-system, "Segoe UI", sans-serif',
        '"JetBrains Mono", "Consolas", monospace',
      ];
    }
    if (/rock|alt|punk|metal|emo|grunge/.test(g)) {
      return [
        '"Bahnschrift", "Arial", sans-serif',
        '"IBM Plex Sans", "Segoe UI", "Arial", sans-serif',
        '"Arial", "Helvetica Neue", sans-serif',
      ];
    }
    if (/electr|house|techno|edm|dnb|synth|trance|dance/.test(g)) {
      return [
        '"Inter", "SF Pro Display", "Segoe UI", "Arial", sans-serif',
        '"JetBrains Mono", "Consolas", monospace',
        '"Manrope", "Segoe UI", "Arial", sans-serif',
      ];
    }
    if (/pop|indie|rnb|soul|jazz|chill|lo-?fi|acoustic/.test(g)) {
      return [
        '"SF Pro Display", -apple-system, "Segoe UI", "Arial", sans-serif',
        '"Manrope", "Segoe UI", "Arial", sans-serif',
        '"Inter", "Arial", sans-serif',
      ];
    }
    return ALL_FONTS;
  }

  function clearWordTimers() {
    for (const t of wordTimers) clearTimeout(t);
    wordTimers = [];
  }

  let lastZoneIdx = -1;

  const SAFE_ZONES = [
    // Левый свободный коридор (между левой панелью и обложкой)
    { xMin: 25, xMax: 32, yMin: 24, yMax: 76 },
    // Правый свободный коридор (между обложкой и правой панелью)
    { xMin: 68, xMax: 75, yMin: 24, yMax: 76 },
    // Верхняя зона над заголовком
    { xMin: 34, xMax: 66, yMin: 11, yMax: 17 },
    // Нижняя зона под громкостью
    { xMin: 34, xMax: 66, yMin: 81, yMax: 89 },
  ];

  function pickSafePos(wPct: number, hPct: number): { x: number; y: number } | null {
    const overlaps = (x: number, y: number) =>
      kineticWords.some(
        (kw) =>
          Math.abs(x - kw.x) < (wPct + kw.wPct) * 0.55 &&
          Math.abs(y - kw.y) < (hPct + kw.hPct) * 0.65
      );

    for (let attempt = 0; attempt < 28; attempt++) {
      const zIdx = (lastZoneIdx + 1 + attempt) % SAFE_ZONES.length;
      const z = SAFE_ZONES[zIdx];
      const x = z.xMin + Math.random() * (z.xMax - z.xMin);
      const y = z.yMin + Math.random() * (z.yMax - z.yMin);
      if (!overlaps(x, y)) {
        lastZoneIdx = zIdx;
        return { x, y };
      }
    }
    return null;
  }

  function spawnKineticWord(text: string) {
    if (kineticWords.length >= 8) return;
    const pool = pickGenreFonts(track?.genre);
    const lenFactor = text.length > 8 ? 0.72 : text.length > 5 ? 0.85 : 1;
    const size = Math.round((20 + Math.random() * 22) * lenFactor);
    const vw = typeof window !== 'undefined' ? Math.max(800, window.innerWidth) : 1280;
    const vh = typeof window !== 'undefined' ? Math.max(600, window.innerHeight) : 720;
    const wPct = Math.min(24, ((text.length * size * 0.65) / vw) * 100 + 3);
    const hPct = Math.min(16, ((size * 1.35) / vh) * 100 + 3);

    const pos = pickSafePos(wPct, hPct);
    if (!pos) return;

    const cols = [
      palette.primary,
      palette.secondary,
      palette.tertiary,
      palette.quaternary,
      '#ffffff',
    ];
    const token: KineticWord = {
      id: wordSeq++,
      text,
      x: pos.x,
      y: pos.y,
      wPct,
      hPct,
      size,
      font: pool[Math.floor(Math.random() * pool.length)],
      italic: Math.random() < 0.35,
      upper: Math.random() < 0.45,
      rot: Math.round((Math.random() - 0.5) * 10),
      color: cols[Math.floor(Math.random() * cols.length)],
    };
    kineticWords = [...kineticWords.slice(-7), token];
    const rmTimer = setTimeout(() => {
      kineticWords = kineticWords.filter((w) => w.id !== token.id);
    }, 2800);
    wordTimers.push(rmTimer);
  }

  $effect(() => {
    if (mode !== 'dynamic-lyrics' || !track) {
      clearWordTimers();
      kineticWords = [];
      return;
    }
    if (lyricsTrackId === track.id) return;
    lyricsTrackId = track.id;
    lyricLines = [];
    lastTriggeredLineIdx = -1;
    clearWordTimers();
    kineticWords = [];

    const targetId = track.id;
    api
      .getTrackLyrics(track)
      .then((res) => {
        if (lyricsTrackId === targetId) {
          lyricLines = res.lines ?? [];
        }
      })
      .catch(() => {});
  });

  $effect(() => {
    if (mode !== 'dynamic-lyrics' || !isPlaying || !track) return;

    const hasSynced = lyricLines.some((l) => l.timeMs !== null && l.text.trim());
    if (hasSynced) {
      const ms = positionSec * 1000 + 350;
      let idx = -1;
      for (let i = 0; i < lyricLines.length; i++) {
        const tm = lyricLines[i].timeMs;
        if (tm !== null && tm <= ms && lyricLines[i].text.trim()) {
          idx = i;
        }
      }
      if (idx >= 0 && idx !== lastTriggeredLineIdx) {
        lastTriggeredLineIdx = idx;
        const words = lyricLines[idx].text.trim().split(/\s+/).filter(Boolean);
        const nextMs = lyricLines[idx + 1]?.timeMs ?? ms + 2200;
        const lineSpan = Math.max(800, Math.min(3500, nextMs - ms));
        const step = Math.min(340, Math.max(140, lineSpan / Math.max(1, words.length)));
        words.forEach((w, wi) => {
          const t = setTimeout(() => {
            if (mode === 'dynamic-lyrics' && isPlaying) {
              spawnKineticWord(w);
            }
          }, wi * step);
          wordTimers.push(t);
        });
      }
    } else if (lyricLines.length > 0) {
      const secBucket = Math.floor(positionSec / 2);
      if (secBucket !== lastTriggeredLineIdx) {
        lastTriggeredLineIdx = secBucket;
        const nonEmpty = lyricLines.filter((l) => l.text.trim());
        const line = nonEmpty[secBucket % Math.max(1, nonEmpty.length)];
        if (line) {
          const words = line.text.trim().split(/\s+/).filter(Boolean).slice(0, 3);
          words.forEach((w, wi) => {
            const t = setTimeout(() => {
              if (mode === 'dynamic-lyrics' && isPlaying) spawnKineticWord(w);
            }, wi * 260);
            wordTimers.push(t);
          });
        }
      }
    } else {
      // Если текста нет — всплывают слова из названия и артиста
      const secBucket = Math.floor(positionSec / 2);
      if (secBucket !== lastTriggeredLineIdx) {
        lastTriggeredLineIdx = secBucket;
        const pool = [
          ...track.title.split(/\s+/),
          ...track.artist.split(/[,&\s]+/),
          track.genre ?? '',
        ].filter((w) => w.length > 1);
        const pick = pool[secBucket % Math.max(1, pool.length)];
        if (pick) spawnKineticWord(pick);
      }
    }
  });

  function readBassEnergy(): number {
    if (!isPlaying) return 0;
    const freq = audioEngine.getFrequencyData();
    if (!freq || freq.length < 8) return 0;
    let sum = 0;
    for (let i = 0; i < 6; i++) sum += freq[i];
    return sum / (6 * 255);
  }

  $effect(() => {
    const isCanvasMode = mode === 'rain-glass' || mode === 'starfield';
    if (!isCanvasMode || !canvasEl) {
      cancelAnimationFrame(rafId);
      return;
    }

    const canvas = canvasEl;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const resize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    resize();
    window.addEventListener('resize', resize);

    const onPointer = (e: PointerEvent) => {
      mouseX = (e.clientX / Math.max(1, window.innerWidth) - 0.5) * 2;
      mouseY = (e.clientY / Math.max(1, window.innerHeight) - 0.5) * 2;
    };
    window.addEventListener('pointermove', onPointer, { passive: true });

    // Инициализация частиц для дождя и космоса
    const drops = Array.from({ length: 85 }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      len: 10 + Math.random() * 24,
      speed: 2.5 + Math.random() * 5.5,
      r: 1 + Math.random() * 2,
    }));

    const starCols = [palette.primary, palette.secondary, palette.tertiary, palette.quaternary, '#ffffff'];
    const stars = Array.from({ length: 140 }, (_, i) => ({
      x: (Math.random() - 0.5) * 2,
      y: (Math.random() - 0.5) * 2,
      z: Math.random(),
      color: starCols[i % starCols.length],
    }));

    let lastRender = 0;

    const render = (now: number = performance.now()) => {
      if (document.hidden || now - lastRender < 33) {
        rafId = requestAnimationFrame(render);
        return;
      }
      lastRender = now;
      const w = canvas.width;
      const h = canvas.height;
      const bass = readBassEnergy();

      ctx.clearRect(0, 0, w, h);

      if (mode === 'rain-glass') {
        for (const d of drops) {
          const jitter = bass > 0.35 ? (Math.random() - 0.5) * bass * 5 : 0;
          d.y += d.speed * (1 + bass * 0.9);
          if (d.y > h + 30) {
            d.y = -30;
            d.x = Math.random() * w;
          }
          ctx.strokeStyle = `rgba(210, 230, 255, ${0.18 + bass * 0.28})`;
          ctx.lineWidth = d.r * 0.75;
          ctx.beginPath();
          ctx.moveTo(d.x + jitter, d.y);
          ctx.lineTo(d.x + jitter, d.y + d.len);
          ctx.stroke();

          ctx.fillStyle = `rgba(255, 255, 255, ${0.25 + bass * 0.35})`;
          ctx.beginPath();
          ctx.arc(d.x + jitter, d.y + d.len, d.r * 0.8, 0, Math.PI * 2);
          ctx.fill();
        }
      } else if (mode === 'starfield') {
        const speed = 0.002 + bass * 0.009;
        for (const s of stars) {
          s.z -= speed;
          if (s.z <= 0.02) {
            s.z = 1;
            s.x = (Math.random() - 0.5) * 2;
            s.y = (Math.random() - 0.5) * 2;
          }
          const px = (s.x / s.z) * (w * 0.45) + w / 2 - mouseX * (1 - s.z) * 42;
          const py = (s.y / s.z) * (h * 0.45) + h / 2 - mouseY * (1 - s.z) * 42;
          const rad = Math.max(0.6, (1 - s.z) * 3.2 * (1 + bass * 0.5));
          if (px >= -10 && px <= w + 10 && py >= -10 && py <= h + 10) {
            ctx.globalAlpha = Math.min(1, (1 - s.z) * 1.15);
            ctx.fillStyle = s.color;
            ctx.beginPath();
            ctx.arc(px, py, rad, 0, Math.PI * 2);
            ctx.fill();
          }
        }
        ctx.globalAlpha = 1;
      }

      rafId = requestAnimationFrame(render);
    };

    rafId = requestAnimationFrame(render);
    return () => {
      cancelAnimationFrame(rafId);
      window.removeEventListener('resize', resize);
      window.removeEventListener('pointermove', onPointer);
    };
  });

  onDestroy(() => {
    cancelAnimationFrame(rafId);
    clearWordTimers();
  });
</script>

{#if mode === 'rain-glass'}
  {#if track?.artwork}
    <div class="rb-layer rb-blur-dark" style={`background-image: url('${track.artwork}')`}></div>
  {/if}
  <canvas class="rb-layer rb-canvas" bind:this={canvasEl}></canvas>
{:else if mode === 'starfield'}
  <div class="rb-layer rb-space-bg"></div>
  <canvas class="rb-layer rb-canvas" bind:this={canvasEl}></canvas>
{:else if mode === 'fog-grain'}
  <div class="rb-layer rb-fog" class:playing={isPlaying}></div>
  <div class="rb-layer rb-grain"></div>
  <div class="rb-layer rb-vignette"></div>
{:else if mode === 'dynamic-lyrics'}
  <div class="rb-layer rb-lyrics-bg">
    {#each kineticWords as kw (kw.id)}
      <span
        class="k-word"
        style={`left: ${kw.x}%; top: ${kw.y}%; font-size: ${kw.size}px; font-family: ${kw.font}; font-style: ${kw.italic ? 'italic' : 'normal'}; text-transform: ${kw.upper ? 'uppercase' : 'none'}; --k-rot: ${kw.rot}deg; --k-col: ${kw.color}`}
      >
        {kw.text}
      </span>
    {/each}
  </div>
{/if}

<style>
  .rb-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
  }

  .rb-canvas {
    width: 100%;
    height: 100%;
  }

  .rb-blur-dark {
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(44px) brightness(0.32) saturate(1.25);
  }

  .rb-space-bg {
    background:
      radial-gradient(circle at 25% 30%, color-mix(in srgb, var(--ambient-2) 18%, transparent), transparent 55%),
      radial-gradient(circle at 75% 70%, color-mix(in srgb, var(--ambient-1) 16%, transparent), transparent 55%),
      #05050a;
  }

  .rb-fog {
    inset: -50px;
    background:
      radial-gradient(circle 480px at 30% 35%, color-mix(in srgb, var(--ambient-1) 42%, transparent), transparent 70%),
      radial-gradient(circle 440px at 70% 60%, color-mix(in srgb, var(--ambient-2) 38%, transparent), transparent 70%),
      radial-gradient(circle 380px at 50% 80%, color-mix(in srgb, var(--ambient-4) 32%, transparent), transparent 72%);
    filter: blur(48px) saturate(1.2);
    opacity: 0.85;
  }

  .rb-fog.playing {
    animation: rb-fog-drift 16s ease-in-out infinite alternate;
  }

  .rb-grain {
    opacity: 0.14;
    background-image: radial-gradient(rgba(255, 255, 255, 0.45) 1px, transparent 0);
    background-size: 3px 3px;
    mix-blend-mode: overlay;
  }

  .rb-vignette {
    background: radial-gradient(circle at 50% 50%, transparent 45%, rgba(0, 0, 0, 0.78) 100%);
  }

  .rb-lyrics-bg {
    overflow: hidden;
    background: radial-gradient(
      circle at 50% 50%,
      color-mix(in srgb, var(--ambient-1) 18%, transparent),
      rgba(6, 6, 10, 0.94) 76%
    );
  }

  .k-word {
    position: absolute;
    font-weight: 900;
    line-height: 1;
    white-space: nowrap;
    color: var(--k-col);
    text-shadow: 0 8px 28px rgba(0, 0, 0, 0.55);
    animation: kinetic-pop 2.7s cubic-bezier(0.22, 1, 0.36, 1) forwards;
    will-change: transform, opacity, filter;
  }

  @keyframes kinetic-pop {
    0% {
      opacity: 0;
      transform: translate(-50%, -44%) scale(0.7) rotate(var(--k-rot));
      filter: blur(8px);
    }
    18% {
      opacity: 0.68;
      transform: translate(-50%, -50%) scale(1.05) rotate(var(--k-rot));
      filter: blur(0px);
    }
    72% {
      opacity: 0.52;
      transform: translate(-50%, -56%) scale(1) rotate(var(--k-rot));
      filter: blur(0px);
    }
    100% {
      opacity: 0;
      transform: translate(-50%, -66%) scale(0.92) rotate(var(--k-rot));
      filter: blur(6px);
    }
  }

  @keyframes rb-fog-drift {
    from {
      transform: translate3d(-2%, -1%, 0) scale(1);
    }
    to {
      transform: translate3d(2%, 2%, 0) scale(1.07);
    }
  }
</style>

<script lang="ts">
  import { Loader2, Music } from 'lucide-svelte';
  import * as api from '../api';
  import type { LyricsResponse, Track } from '../types';

  interface Props {
    track: Track;
    positionSec: number;
    isPlaying: boolean;
    onSeek: (sec: number) => void;
  }

  let {
    track,
    positionSec,
    isPlaying,
    onSeek,
  }: Props = $props();

  let lyrics = $state<LyricsResponse | null>(null);
  let loading = $state(false);
  let scrollEl: HTMLDivElement | undefined = $state();
  let lineEls = $state<(HTMLButtonElement | null)[]>([]);
  let smoothMs = $state(0);

  let lastTrackId = '';

  // Плавное обновление времени (~20 fps вместо 60-144 fps) чтобы не нагружать WebView2
  $effect(() => {
    const baseMs = positionSec * 1000;
    smoothMs = baseMs;
    if (!isPlaying) return;

    const startPerf = performance.now();
    let lastPush = startPerf;
    let raf = 0;
    function tick(now: number) {
      if (now - lastPush >= 50) {
        lastPush = now;
        smoothMs = baseMs + Math.min(980, now - startPerf);
      }
      raf = requestAnimationFrame(tick);
    }
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    const current = track;
    if (!current || current.id === lastTrackId) return;
    lastTrackId = current.id;
    let cancelled = false;
    loading = true;
    lyrics = null;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    const timeoutPromise = new Promise<LyricsResponse>((resolve) => {
      timeoutId = setTimeout(
        () => resolve({ synced: false, source: 'Таймаут', lines: [] }),
        9000
      );
    });
    Promise.race([api.getTrackLyrics(current), timeoutPromise])
      .then((res) => {
        if (!cancelled) lyrics = res;
      })
      .catch(() => {
        if (!cancelled) lyrics = { synced: false, source: 'Ошибка', lines: [] };
      })
      .finally(() => {
        if (timeoutId) clearTimeout(timeoutId);
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  const activeIdx = $derived.by(() => {
    if (!lyrics || !lyrics.synced || lyrics.lines.length === 0) return -1;
    let idx = -1;
    for (let i = 0; i < lyrics.lines.length; i++) {
      const t = lyrics.lines[i].timeMs;
      if (t !== null && t <= smoothMs + 180) {
        idx = i;
      } else if (t !== null && t > smoothMs + 180) {
        break;
      }
    }
    return idx;
  });

  const activeProgressPct = $derived.by(() => {
    if (!lyrics || !lyrics.synced || activeIdx < 0) return 100;
    const cur = lyrics.lines[activeIdx]?.timeMs ?? 0;
    const next = lyrics.lines[activeIdx + 1]?.timeMs ?? cur + 4000;
    const span = Math.max(400, next - cur);
    return Math.max(0, Math.min(100, ((smoothMs - cur) / span) * 100));
  });

  $effect(() => {
    const idx = activeIdx;
    if (idx < 0 || !scrollEl) return;
    const el = lineEls[idx];
    if (el) {
      const targetTop = el.offsetTop - scrollEl.clientHeight / 2 + el.clientHeight / 2;
      scrollEl.scrollTo({ top: Math.max(0, targetTop), behavior: 'smooth' });
    }
  });
</script>

<div class="lyrics-panel">
  {#if loading}
    <div class="l-state">
      <Loader2 size={18} class="spin" />
      <span>Ищем текст песни...</span>
    </div>
  {:else if !lyrics || lyrics.lines.length === 0}
    <div class="l-state">
      <Music size={18} />
      <span>Текст для этого трека не найден</span>
    </div>
  {:else}
    <div class="l-scroll" bind:this={scrollEl}>
      <div class="l-spacer"></div>
      {#each lyrics.lines as line, idx (idx)}
        {@const isActive = lyrics.synced && idx === activeIdx}
        {@const dist = activeIdx >= 0 ? Math.abs(idx - activeIdx) : 0}
        {@const words = line.text.split(' ')}
        <button
          type="button"
          bind:this={lineEls[idx]}
          class="l-line"
          class:active={isActive}
          class:past={lyrics.synced && activeIdx >= 0 && idx < activeIdx}
          class:far={lyrics.synced && dist >= 2}
          class:clickable={line.timeMs !== null}
          onclick={() => {
            if (line.timeMs !== null) {
              onSeek(Math.floor(line.timeMs / 1000));
            }
          }}
        >
          {#if isActive && words.length > 0}
            {#each words as word, wIdx (wIdx)}
              {@const wStart = (wIdx / words.length) * 100}
              {@const wEnd = ((wIdx + 1) / words.length) * 100}
              {@const wFill = Math.max(0, Math.min(100, ((activeProgressPct - wStart) / Math.max(1, wEnd - wStart)) * 100))}
              <span
                class="l-word"
                class:lit={wFill > 20}
                style="--wfill: {wFill.toFixed(1)}%"
              >{word}{wIdx < words.length - 1 ? ' ' : ''}</span>
            {/each}
          {:else}
            {line.text}
          {/if}
        </button>
      {/each}
      <div class="l-source">Источник: {lyrics.source}{lyrics.synced ? ' · Synced' : ''}</div>
      <div class="l-spacer"></div>
    </div>
  {/if}
</div>

<style>
  .lyrics-panel {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    mask-image: linear-gradient(transparent 0%, black 22%, black 78%, transparent 100%);
    -webkit-mask-image: linear-gradient(transparent 0%, black 22%, black 78%, transparent 100%);
  }

  .l-scroll {
    position: relative;
    flex: 1;
    overflow-y: auto;
    padding: 0 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    scrollbar-width: none;
  }

  .l-scroll::-webkit-scrollbar {
    display: none;
  }

  .l-spacer {
    height: 52px;
    flex-shrink: 0;
  }

  .l-line {
    font-family: 'SF Pro Display', 'SF Pro Text', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    border: none;
    background: transparent;
    text-align: center;
    font-size: 20px;
    font-weight: 700;
    line-height: 1.32;
    letter-spacing: -0.025em;
    color: rgba(255, 255, 255, 0.52);
    transform-origin: center center;
    transform: scale(0.94);
    transition:
      transform 0.35s cubic-bezier(0.22, 1, 0.36, 1),
      color 0.25s ease,
      opacity 0.25s ease;
    padding: 2px 6px;
  }

  .l-line.clickable {
    cursor: pointer;
  }

  .l-line.clickable:hover {
    color: rgba(255, 255, 255, 0.88);
  }

  .l-line.far {
    opacity: 0.42;
  }

  .l-line.past {
    color: rgba(255, 255, 255, 0.34);
  }

  .l-line.active {
    font-size: 25px;
    font-weight: 800;
    transform: scale(1.04);
    opacity: 1;
    color: rgba(255, 255, 255, 0.96);
  }

  .l-word {
    display: inline-block;
    white-space: pre;
    background: linear-gradient(
      90deg,
      #ffffff var(--wfill, 0%),
      rgba(255, 255, 255, 0.38) var(--wfill, 0%)
    );
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .l-word.lit {
    text-shadow: 0 0 12px color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .l-source {
    font-size: 9px;
    color: rgba(255, 255, 255, 0.45);
    margin-top: 4px;
  }

  .l-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  :global(.spin) {
    animation: spin-anim 1s linear infinite;
  }

  @keyframes spin-anim {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .l-line {
      transition: none;
      filter: none;
    }
  }
</style>

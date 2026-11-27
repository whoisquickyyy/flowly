<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { WeatherAtmosphere } from '../types';

  interface Props {
    atmosphere: WeatherAtmosphere | null;
    hour: number;
  }

  let { atmosphere, hour }: Props = $props();
  let canvasEl: HTMLCanvasElement | undefined = $state();
  let rafId = 0;

  $effect(() => {
    const canvas = canvasEl;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const kind = atmosphere?.weatherKind ?? 'clear';
    const isNight = hour >= 22 || hour < 6;

    const resize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    resize();
    window.addEventListener('resize', resize);

    // Частицы в зависимости от погоды
    const flakes = Array.from({ length: 90 }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      r: 1.4 + Math.random() * 2.8,
      vy: 0.7 + Math.random() * 1.4,
      vx: (Math.random() - 0.5) * 0.6,
      phase: Math.random() * Math.PI * 2,
      alpha: 0.35 + Math.random() * 0.5,
    }));

    const drops = Array.from({ length: kind === 'storm' ? 140 : 95 }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      len: 12 + Math.random() * 20,
      vy: 9 + Math.random() * 7,
      vx: -1.8,
      alpha: 0.18 + Math.random() * 0.25,
    }));

    const stars = Array.from({ length: 90 }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height * 0.75,
      r: 0.7 + Math.random() * 1.6,
      phase: Math.random() * Math.PI * 2,
    }));

    const bokeh = Array.from({ length: 24 }, () => ({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      r: 18 + Math.random() * 44,
      vx: (Math.random() - 0.5) * 0.25,
      vy: -0.15 - Math.random() * 0.25,
      alpha: 0.03 + Math.random() * 0.05,
    }));

    let lightningAlpha = 0;
    let nextLightningAt = performance.now() + 3000 + Math.random() * 6000;
    let lastRender = 0;

    const render = (now: number) => {
      if (document.hidden || now - lastRender < 33) {
        rafId = requestAnimationFrame(render);
        return;
      }
      lastRender = now;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);

      if (kind === 'snow') {
        for (const f of flakes) {
          f.phase += 0.015;
          f.y += f.vy;
          f.x += f.vx + Math.sin(f.phase) * 0.45;
          if (f.y > h + 10) {
            f.y = -10;
            f.x = Math.random() * w;
          }
          if (f.x > w + 10) f.x = -10;
          if (f.x < -10) f.x = w + 10;

          ctx.fillStyle = `rgba(235, 246, 255, ${f.alpha})`;
          ctx.beginPath();
          ctx.arc(f.x, f.y, f.r, 0, Math.PI * 2);
          ctx.fill();
        }
      } else if (kind === 'rain' || kind === 'storm') {
        if (kind === 'storm') {
          if (now >= nextLightningAt) {
            lightningAlpha = 0.28;
            nextLightningAt = now + 4500 + Math.random() * 8500;
          }
          if (lightningAlpha > 0.005) {
            ctx.fillStyle = `rgba(210, 228, 255, ${lightningAlpha})`;
            ctx.fillRect(0, 0, w, h);
            lightningAlpha *= 0.86;
          }
        }

        ctx.lineWidth = 1.1;
        for (const d of drops) {
          d.x += d.vx;
          d.y += d.vy;
          if (d.y > h + 24) {
            d.y = -24;
            d.x = Math.random() * (w + 120);
          }
          ctx.strokeStyle = `rgba(186, 220, 255, ${d.alpha})`;
          ctx.beginPath();
          ctx.moveTo(d.x, d.y);
          ctx.lineTo(d.x + d.vx * 1.4, d.y + d.len);
          ctx.stroke();
        }
      } else if (kind === 'clear' && isNight) {
        for (const s of stars) {
          s.phase += 0.02;
          const a = 0.25 + (Math.sin(s.phase) + 1) * 0.32;
          ctx.fillStyle = `rgba(230, 238, 255, ${a})`;
          ctx.beginPath();
          ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
          ctx.fill();
        }
      } else if (kind === 'clear') {
        const warm = hour >= 17 ? '255, 160, 120' : '255, 225, 150';
        for (const b of bokeh) {
          b.x += b.vx;
          b.y += b.vy;
          if (b.y < -b.r) {
            b.y = h + b.r;
            b.x = Math.random() * w;
          }
          ctx.fillStyle = `rgba(${warm}, ${b.alpha})`;
          ctx.beginPath();
          ctx.arc(b.x, b.y, b.r, 0, Math.PI * 2);
          ctx.fill();
        }
      } else {
        // cloudy / fog: мягкие дрейфующие слои тумана/облаков
        for (const b of bokeh) {
          b.x += b.vx * 1.4;
          if (b.x > w + b.r * 2) b.x = -b.r * 2;
          if (b.x < -b.r * 2) b.x = w + b.r * 2;
          ctx.fillStyle = `rgba(210, 220, 240, ${b.alpha * 0.7})`;
          ctx.beginPath();
          ctx.arc(b.x, b.y, b.r * 1.9, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      rafId = requestAnimationFrame(render);
    };

    rafId = requestAnimationFrame(render);
    return () => {
      cancelAnimationFrame(rafId);
      window.removeEventListener('resize', resize);
    };
  });

  onDestroy(() => cancelAnimationFrame(rafId));
</script>

<canvas class="weather-canvas" bind:this={canvasEl}></canvas>

<style>
  .weather-canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
</style>

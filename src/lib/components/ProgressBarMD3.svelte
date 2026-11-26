<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    positionSec: number;
    durationSec: number;
    isPlaying: boolean;
    onSeek: (sec: number) => void;
  }

  let { positionSec, durationSec, isPlaying, onSeek }: Props = $props();

  let trackEl: HTMLDivElement | undefined = $state();
  let trackWidth = $state(280);
  let dragging = $state(false);
  let hoverPct = $state<number | null>(null);
  let dragPct = $state(0);
  let phase = $state(0);

  const displayPct = $derived.by(() => {
    if (dragging) return dragPct;
    if (durationSec <= 0) return 0;
    return Math.max(0, Math.min(100, (positionSec / durationSec) * 100));
  });

  const displaySec = $derived(
    dragging && durationSec > 0 ? Math.round((dragPct / 100) * durationSec) : positionSec
  );

  function formatTime(sec: number): string {
    const s = Math.max(0, Math.floor(sec));
    const m = Math.floor(s / 60);
    const rem = s % 60;
    return `${m}:${rem.toString().padStart(2, '0')}`;
  }

  function pctFromEvent(e: PointerEvent): number {
    if (!trackEl) return 0;
    const rect = trackEl.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    return Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100));
  }

  function handlePointerDown(e: PointerEvent) {
    if (durationSec <= 0) return;
    dragging = true;
    dragPct = pctFromEvent(e);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    const pct = pctFromEvent(e);
    hoverPct = pct;
    if (dragging) {
      dragPct = pct;
    }
  }

  function handlePointerUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    const pct = pctFromEvent(e);
    if (durationSec > 0) {
      onSeek(Math.floor((pct / 100) * durationSec));
    }
  }

  onMount(() => {
    let raf = 0;
    let last = performance.now();

    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      if (isPlaying) {
        phase = (phase + dt * 5.2) % (Math.PI * 2);
      }
      if (trackEl) {
        const w = trackEl.clientWidth;
        if (w > 0 && Math.abs(w - trackWidth) > 1) {
          trackWidth = w;
        }
      }
      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  const activeEndX = $derived.by(() => {
    const thumbX = (displayPct / 100) * trackWidth;
    return Math.max(0, thumbX - 7);
  });

  const inactiveStartX = $derived.by(() => {
    const thumbX = (displayPct / 100) * trackWidth;
    return Math.min(trackWidth, thumbX + 7);
  });

  const wavyPath = $derived.by(() => {
    const endX = activeEndX;
    if (endX <= 2) return '';
    const midY = 12;
    const amp = isPlaying ? 3.2 : 0.6;
    const wavelength = 18;
    const step = 3;
    let d = `M 0 ${(midY + Math.sin(phase) * amp).toFixed(2)}`;
    for (let x = step; x <= endX; x += step) {
      const y = midY + Math.sin((x / wavelength) * Math.PI * 2 - phase) * amp;
      d += ` L ${x.toFixed(1)} ${y.toFixed(2)}`;
    }
    const lastY = midY + Math.sin((endX / wavelength) * Math.PI * 2 - phase) * amp;
    d += ` L ${endX.toFixed(1)} ${lastY.toFixed(2)}`;
    return d;
  });
</script>

<div class="md3-progress-wrap" class:dragging>
  <span class="time-lbl">{formatTime(displaySec)}</span>

  <div
    class="md3-track-hit"
    bind:this={trackEl}
    role="slider"
    tabindex="0"
    aria-label="Перемотка трека"
    aria-valuenow={displaySec}
    aria-valuemin={0}
    aria-valuemax={durationSec}
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={() => (dragging = false)}
    onpointerleave={() => (hoverPct = null)}
    onkeydown={(e) => {
      if (e.key === 'ArrowRight') onSeek(Math.min(durationSec, positionSec + 5));
      if (e.key === 'ArrowLeft') onSeek(Math.max(0, positionSec - 5));
    }}
  >
    <svg class="md3-svg" width="100%" height="24" viewBox="0 0 {trackWidth} 24" preserveAspectRatio="none">
      {#if inactiveStartX < trackWidth - 2}
        <line
          x1={inactiveStartX}
          y1="12"
          x2={trackWidth}
          y2="12"
          stroke="rgba(255, 255, 255, 0.16)"
          stroke-width="4"
          stroke-linecap="round"
        />
      {/if}

      {#if wavyPath}
        <path
          d={wavyPath}
          fill="none"
          stroke="var(--accent)"
          stroke-width="3.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      {/if}
    </svg>

    <div class="md3-thumb" style="left: {displayPct}%"></div>

    {#if hoverPct !== null && durationSec > 0}
      <div class="hover-tip" style="left: {hoverPct}%">
        {formatTime(Math.round((hoverPct / 100) * durationSec))}
      </div>
    {/if}
  </div>

  <span class="time-lbl right">{formatTime(durationSec)}</span>
</div>

<style>
  .md3-progress-wrap {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
  }

  .time-lbl {
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.78);
    text-shadow: 0 1px 8px rgba(0, 0, 0, 0.65);
    font-variant-numeric: tabular-nums;
    min-width: 34px;
    flex-shrink: 0;
    transition: color 0.15s ease;
  }

  .time-lbl.right {
    text-align: right;
  }

  .md3-progress-wrap.dragging .time-lbl,
  .md3-progress-wrap:hover .time-lbl {
    color: var(--text-primary);
  }

  .md3-track-hit {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 28px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
    outline: none;
  }

  .md3-track-hit:focus,
  .md3-track-hit:focus-visible {
    outline: none;
  }

  .md3-svg {
    display: block;
    overflow: visible;
    pointer-events: none;
  }

  .md3-thumb {
    position: absolute;
    top: 50%;
    width: 4px;
    height: 18px;
    border-radius: 999px;
    background: var(--text-primary);
    transform: translate(-50%, -50%);
    pointer-events: none;
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 55%, transparent);
    transition: width 0.16s cubic-bezier(0.2, 0, 0, 1), height 0.16s cubic-bezier(0.2, 0, 0, 1), background-color 0.16s ease;
  }

  .md3-track-hit:hover .md3-thumb,
  .md3-progress-wrap.dragging .md3-thumb {
    width: 6px;
    height: 24px;
    background: #ffffff;
    box-shadow: 0 0 14px var(--accent);
  }

  .hover-tip {
    position: absolute;
    bottom: 30px;
    transform: translateX(-50%);
    padding: 2px 7px;
    border-radius: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    color: var(--text-primary);
    font-size: 10px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    white-space: nowrap;
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.55);
  }
</style>

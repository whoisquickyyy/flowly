<script lang="ts">
  interface Props {
    text: string;
    paused?: boolean;
  }

  let { text, paused = false }: Props = $props();

  let wrapEl: HTMLDivElement | undefined = $state();
  let measureEl: HTMLSpanElement | undefined = $state();
  let overflowing = $state(false);
  let durationSec = $state(12);

  $effect(() => {
    void text;
    if (!wrapEl || !measureEl) return;
    const check = () => {
      if (!wrapEl || !measureEl) return;
      const diff = measureEl.scrollWidth - wrapEl.clientWidth;
      overflowing = diff > 6;
      if (overflowing) {
        durationSec = Math.max(8, Math.min(26, measureEl.scrollWidth / 34));
      }
    };
    const raf = requestAnimationFrame(check);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div
  class="marquee-wrap"
  class:moving={overflowing && !paused}
  bind:this={wrapEl}
  style="--m-dur: {durationSec}s"
  title={text}
>
  <span class="marquee-measure" bind:this={measureEl}>{text}</span>
  {#if overflowing && !paused}
    <div class="marquee-track">
      <span>{text}</span>
      <span aria-hidden="true">{text}</span>
    </div>
  {:else}
    <span class="marquee-static">{text}</span>
  {/if}
</div>

<style>
  .marquee-wrap {
    position: relative;
    overflow: hidden;
    width: 100%;
    min-width: 0;
  }

  .marquee-wrap.moving {
    -webkit-mask-image: linear-gradient(90deg, transparent, #000 10px, #000 calc(100% - 10px), transparent);
    mask-image: linear-gradient(90deg, transparent, #000 10px, #000 calc(100% - 10px), transparent);
  }

  .marquee-measure {
    position: absolute;
    visibility: hidden;
    white-space: nowrap;
    pointer-events: none;
  }

  .marquee-static {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .marquee-track {
    display: inline-flex;
    width: max-content;
    gap: 36px;
    white-space: nowrap;
    animation: marquee-scroll var(--m-dur, 12s) linear 1.2s infinite;
  }

  .marquee-wrap:hover .marquee-track {
    animation-play-state: paused;
  }

  @keyframes marquee-scroll {
    0%, 10% {
      transform: translateX(0);
    }
    90%, 100% {
      transform: translateX(calc(-50% - 18px));
    }
  }
</style>

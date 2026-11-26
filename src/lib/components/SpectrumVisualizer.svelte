<script lang="ts">
  import { audioEngine } from '../audioEngine';

  interface Props {
    isPlaying: boolean;
    accentColor: string;
    bands: number[];
    eqEnabled: boolean;
    height?: number;
  }

  let { isPlaying, accentColor, bands, eqEnabled, height = 44 }: Props = $props();
  let canvasEl: HTMLCanvasElement | undefined = $state();

  $effect(() => {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext('2d');
    if (!ctx) return;

    const barCount = 36;
    const values = new Float32Array(barCount).fill(0.08);
    let raf = 0;
    let phase = 0;

    function render() {
      if (!canvasEl || !ctx) return;
      const dpr = window.devicePixelRatio || 1;
      const w = canvasEl.clientWidth;
      const h = canvasEl.clientHeight;
      if (canvasEl.width !== w * dpr || canvasEl.height !== h * dpr) {
        canvasEl.width = w * dpr;
        canvasEl.height = h * dpr;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      phase += isPlaying ? 0.08 : 0.015;
      const gap = 3;
      const barW = Math.max(2, (w - gap * (barCount - 1)) / barCount);

      const fft = isPlaying ? audioEngine.getFrequencyData() : null;
      let hasSignal = false;
      if (fft) {
        for (let k = 0; k < fft.length; k++) {
          if (fft[k] > 2) {
            hasSignal = true;
            break;
          }
        }
      }

      for (let i = 0; i < barCount; i++) {
        const bandIdx = Math.min(9, Math.floor((i / barCount) * 10));
        const eqBoost = eqEnabled ? 1 + (bands[bandIdx] ?? 0) / 18 : 1;

        let target = 0.08;
        if (isPlaying && hasSignal && fft) {
          const bin = Math.min(fft.length - 1, Math.floor((i / barCount) * (fft.length * 0.75)));
          target = Math.max(0.08, Math.min(1, fft[bin] / 235));
        } else if (isPlaying) {
          target = Math.min(
            1,
            Math.max(
              0.08,
              (0.28 +
                0.35 * Math.abs(Math.sin(phase + i * 0.37)) +
                0.28 * Math.abs(Math.cos(phase * 1.4 - i * 0.21))) *
                eqBoost
            )
          );
        }

        values[i] += (target - values[i]) * 0.22;

        const barH = Math.max(3, values[i] * (h - 4));
        const x = i * (barW + gap);
        const y = h - barH;

        const grad = ctx.createLinearGradient(0, y, 0, h);
        grad.addColorStop(0, accentColor);
        grad.addColorStop(1, 'rgba(255,255,255,0.12)');
        ctx.fillStyle = grad;

        ctx.beginPath();
        ctx.roundRect(x, y, barW, barH, 2);
        ctx.fill();
      }

      raf = requestAnimationFrame(render);
    }

    raf = requestAnimationFrame(render);
    return () => cancelAnimationFrame(raf);
  });
</script>

<canvas
  bind:this={canvasEl}
  class="visualizer"
  style="height: {height}px"
></canvas>

<style>
  .visualizer {
    width: 100%;
    display: block;
    border-radius: 8px;
  }
</style>

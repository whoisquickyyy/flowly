import type { AudioEffectsConfig, CrossfadeCurve, MixPreset } from './types';

const EQ_FREQS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

class AudioEngine {
  private ctx: AudioContext | null = null;
  private sourceA: MediaElementAudioSourceNode | null = null;
  private sourceB: MediaElementAudioSourceNode | null = null;
  private deckGainA: GainNode | null = null;
  private deckGainB: GainNode | null = null;
  private deckFilterA: BiquadFilterNode | null = null;
  private deckFilterB: BiquadFilterNode | null = null;
  private preampGain: GainNode | null = null;
  private filters: BiquadFilterNode[] = [];
  private bassFilter: BiquadFilterNode | null = null;
  private compressor: DynamicsCompressorNode | null = null;
  private limiter: DynamicsCompressorNode | null = null;
  private makeupGain: GainNode | null = null;
  private dryGain: GainNode | null = null;
  private wetGain: GainNode | null = null;
  private delayL: DelayNode | null = null;
  private delayR: DelayNode | null = null;
  private analyser: AnalyserNode | null = null;
  private freqBuf: Uint8Array<ArrayBuffer> | null = null;
  private initialized = false;
  private pendingSinkId = 'default';
  private beatmatchRaf = new WeakMap<HTMLAudioElement, number>();

  private setPreservesPitch(el: HTMLAudioElement, enabled: boolean) {
    const anyEl = el as HTMLAudioElement & {
      preservesPitch?: boolean;
      webkitPreservesPitch?: boolean;
      mozPreservesPitch?: boolean;
    };
    if ('preservesPitch' in anyEl) anyEl.preservesPitch = enabled;
    if ('webkitPreservesPitch' in anyEl) anyEl.webkitPreservesPitch = enabled;
    if ('mozPreservesPitch' in anyEl) anyEl.mozPreservesPitch = enabled;
  }

  cancelBeatmatch(el: HTMLAudioElement | undefined | null) {
    if (!el) return;
    const raf = this.beatmatchRaf.get(el);
    if (raf !== undefined) {
      cancelAnimationFrame(raf);
      this.beatmatchRaf.delete(el);
    }
    this.setPreservesPitch(el, true);
    if (Math.abs(el.playbackRate - 1) > 0.001) {
      el.playbackRate = 1;
    }
  }

  /**
   * Time-Stretch Beatmatching:
   * Подстраивает темп входящего трека (playbackRate с сохранением тональности)
   * под BPM уходящего трека и плавно возвращает к 1.0 после перехода.
   */
  beatmatchTransition(
    incomingEl: HTMLAudioElement,
    outgoingBpm: number,
    incomingBpm: number,
    crossfadeSec: number
  ) {
    this.cancelBeatmatch(incomingEl);
    if (!outgoingBpm || !incomingBpm || crossfadeSec <= 0.2) return;

    const candidates = [
      outgoingBpm / incomingBpm,
      outgoingBpm / (incomingBpm * 2),
      (outgoingBpm * 2) / incomingBpm,
    ];
    let bestRatio = 1;
    let bestDiff = Infinity;
    for (const r of candidates) {
      const d = Math.abs(1 - r);
      if (d < bestDiff) {
        bestDiff = d;
        bestRatio = r;
      }
    }

    // Применяем таймстретч только в музыкальном диапазоне ±12%, чтобы не портить звук
    if (bestRatio < 0.88 || bestRatio > 1.12 || Math.abs(bestRatio - 1) < 0.008) {
      return;
    }

    this.setPreservesPitch(incomingEl, true);
    incomingEl.playbackRate = bestRatio;

    const holdMs = Math.max(800, crossfadeSec * 600);
    const glideMs = Math.max(1800, crossfadeSec * 700 + 1800);
    const startMs = performance.now();

    const step = (now: number) => {
      const elapsed = now - startMs;
      if (elapsed <= holdMs) {
        incomingEl.playbackRate = bestRatio;
      } else {
        const t = Math.min(1, (elapsed - holdMs) / glideMs);
        // Плавная S-кривая возврата к 1.0
        const eased = t * t * (3 - 2 * t);
        incomingEl.playbackRate = bestRatio + (1 - bestRatio) * eased;
        if (t >= 1) {
          incomingEl.playbackRate = 1;
          this.beatmatchRaf.delete(incomingEl);
          return;
        }
      }
      this.beatmatchRaf.set(incomingEl, requestAnimationFrame(step));
    };

    this.beatmatchRaf.set(incomingEl, requestAnimationFrame(step));
  }

  initDecks(elA: HTMLAudioElement, elB: HTMLAudioElement) {
    if (this.initialized && this.ctx) return;

    const AudioCtx =
      window.AudioContext ||
      (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtx) return;

    try {
      this.ctx = new AudioCtx();
      if (this.pendingSinkId && this.pendingSinkId !== 'default' && 'setSinkId' in this.ctx) {
        const ctxWithSink = this.ctx as AudioContext & { setSinkId?: (id: string) => Promise<void> };
        void ctxWithSink.setSinkId?.(this.pendingSinkId).catch(() => {});
      }
      this.sourceA = this.ctx.createMediaElementSource(elA);
      this.sourceB = this.ctx.createMediaElementSource(elB);
      this.deckGainA = this.ctx.createGain();
      this.deckGainB = this.ctx.createGain();
      this.deckGainA.gain.value = 1;
      this.deckGainB.gain.value = 0;

      this.deckFilterA = this.ctx.createBiquadFilter();
      this.deckFilterB = this.ctx.createBiquadFilter();
      this.deckFilterA.type = 'allpass';
      this.deckFilterB.type = 'allpass';

      const mixBus = this.ctx.createGain();
      mixBus.gain.value = 1;
      this.sourceA.connect(this.deckFilterA);
      this.deckFilterA.connect(this.deckGainA);
      this.sourceB.connect(this.deckFilterB);
      this.deckFilterB.connect(this.deckGainB);
      this.deckGainA.connect(mixBus);
      this.deckGainB.connect(mixBus);

      this.initialized = true;

      // Preamp (-24..+6 dB) before EQ filters
      this.preampGain = this.ctx.createGain();
      this.preampGain.gain.value = 1.0;

      // 10-band EQ
      this.filters = EQ_FREQS.map((freq, idx) => {
        const f = this.ctx!.createBiquadFilter();
        f.frequency.value = freq;
        if (idx === 0) {
          f.type = 'lowshelf';
        } else if (idx === EQ_FREQS.length - 1) {
          f.type = 'highshelf';
        } else {
          f.type = 'peaking';
          f.Q.value = 1.4;
        }
        f.gain.value = 0;
        return f;
      });

      // Dedicated Bass Boost (75 Hz lowshelf)
      this.bassFilter = this.ctx.createBiquadFilter();
      this.bassFilter.type = 'lowshelf';
      this.bassFilter.frequency.value = 75;
      this.bassFilter.gain.value = 0;

      // Volume Normalization (Compressor + Makeup)
      this.compressor = this.ctx.createDynamicsCompressor();
      this.compressor.threshold.value = -16;
      this.compressor.knee.value = 18;
      this.compressor.ratio.value = 1;
      this.compressor.attack.value = 0.004;
      this.compressor.release.value = 0.22;

      this.makeupGain = this.ctx.createGain();
      this.makeupGain.gain.value = 1.0;

      // Spatial 3D stage (True Mid-Side stereo widening + Binaural early-reflection room)
      this.dryGain = this.ctx.createGain();
      this.dryGain.gain.value = 1.0;

      this.wetGain = this.ctx.createGain();
      this.wetGain.gain.value = 0.0;

      const splitter = this.ctx.createChannelSplitter(2);
      const merger = this.ctx.createChannelMerger(2);

      // Mid = 0.5*L + 0.5*R
      const midL = this.ctx.createGain();
      const midR = this.ctx.createGain();
      midL.gain.value = 0.5;
      midR.gain.value = 0.5;
      const midOut = this.ctx.createGain();
      midOut.gain.value = 0.85;

      // Side = 0.5*L - 0.5*R -> Air shelf -> Boost 1.9x
      const sideL = this.ctx.createGain();
      const sideR = this.ctx.createGain();
      sideL.gain.value = 0.5;
      sideR.gain.value = -0.5;
      const sideAir = this.ctx.createBiquadFilter();
      sideAir.type = 'highshelf';
      sideAir.frequency.value = 5500;
      sideAir.gain.value = 4.0;
      const sideBoost = this.ctx.createGain();
      sideBoost.gain.value = 1.9;
      const sideInvert = this.ctx.createGain();
      sideInvert.gain.value = -1.0;

      // Short decorrelated binaural room impulse (75ms)
      const roomConv = this.ctx.createConvolver();
      const sr = this.ctx.sampleRate;
      const irLen = Math.max(1, Math.floor(sr * 0.075));
      const irBuf = this.ctx.createBuffer(2, irLen, sr);
      const preDelay = Math.floor(sr * 0.004);
      for (let ch = 0; ch < 2; ch++) {
        const chData = irBuf.getChannelData(ch);
        for (let i = preDelay; i < irLen; i++) {
          const t = (i - preDelay) / (irLen - preDelay);
          const env = Math.pow(1 - t, 2.5);
          chData[i] = (Math.random() * 2 - 1) * env * 0.35;
        }
      }
      roomConv.buffer = irBuf;
      const roomFilter = this.ctx.createBiquadFilter();
      roomFilter.type = 'lowpass';
      roomFilter.frequency.value = 5200;
      const roomGain = this.ctx.createGain();
      roomGain.gain.value = 0.28;

      // Output Soft Limiter (anti-clipping protection)
      this.limiter = this.ctx.createDynamicsCompressor();
      this.limiter.threshold.value = -0.8;
      this.limiter.knee.value = 3.0;
      this.limiter.ratio.value = 20.0;
      this.limiter.attack.value = 0.002;
      this.limiter.release.value = 0.08;

      // Analyser for live FFT
      this.analyser = this.ctx.createAnalyser();
      this.analyser.fftSize = 128;
      this.analyser.smoothingTimeConstant = 0.78;
      this.freqBuf = new Uint8Array(new ArrayBuffer(this.analyser.frequencyBinCount));

      // Chain: mixBus -> preampGain -> filters[0..9] -> bassFilter -> compressor -> makeupGain
      let node: AudioNode = this.preampGain;
      mixBus.connect(this.preampGain);
      for (const f of this.filters) {
        node.connect(f);
        node = f;
      }
      node.connect(this.bassFilter);
      this.bassFilter.connect(this.compressor);
      this.compressor.connect(this.makeupGain);

      // Dry path (transparent bypass when Spatial is OFF)
      this.makeupGain.connect(this.dryGain);
      this.dryGain.connect(this.limiter);

      // Wet spatial path (M/S widener + binaural early reflections)
      this.makeupGain.connect(splitter);
      splitter.connect(midL, 0);
      splitter.connect(midR, 1);
      midL.connect(midOut);
      midR.connect(midOut);

      splitter.connect(sideL, 0);
      splitter.connect(sideR, 1);
      sideL.connect(sideAir);
      sideR.connect(sideAir);
      sideAir.connect(sideBoost);

      // L = Mid + Side, R = Mid - Side
      midOut.connect(merger, 0, 0);
      sideBoost.connect(merger, 0, 0);
      midOut.connect(merger, 0, 1);
      sideBoost.connect(sideInvert);
      sideInvert.connect(merger, 0, 1);

      merger.connect(this.wetGain);
      this.makeupGain.connect(roomConv);
      roomConv.connect(roomFilter);
      roomFilter.connect(roomGain);
      roomGain.connect(this.wetGain);

      this.wetGain.connect(this.limiter);

      this.limiter.connect(this.analyser);
      this.analyser.connect(this.ctx.destination);
    } catch (e) {
      console.warn('Web Audio API init failed:', e);
    }
  }

  crossfadeTo(
    target: 'A' | 'B',
    durationSec: number,
    preset: MixPreset = 'fade',
    curve: CrossfadeCurve = 'equal-power'
  ) {
    if (!this.ctx || !this.deckGainA || !this.deckGainB || !this.deckFilterA || !this.deckFilterB) return;
    const now = this.ctx.currentTime;
    const inGain = target === 'A' ? this.deckGainA.gain : this.deckGainB.gain;
    const outGain = target === 'A' ? this.deckGainB.gain : this.deckGainA.gain;
    const inFilter = target === 'A' ? this.deckFilterA : this.deckFilterB;
    const outFilter = target === 'A' ? this.deckFilterB : this.deckFilterA;

    try {
      inGain.cancelAndHoldAtTime?.(now);
      outGain.cancelAndHoldAtTime?.(now);
    } catch {
      // ignore if unsupported
    }
    inGain.cancelScheduledValues(0);
    outGain.cancelScheduledValues(0);
    inFilter.frequency.cancelScheduledValues(0);
    inFilter.gain.cancelScheduledValues(0);
    outFilter.frequency.cancelScheduledValues(0);
    outFilter.gain.cancelScheduledValues(0);

    if (durationSec <= 0.1) {
      inGain.setValueAtTime(1, now);
      outGain.setValueAtTime(0, now);
      inFilter.type = 'allpass';
      outFilter.type = 'allpass';
      return;
    }

    const steps = 32;
    const inCurve = new Float32Array(steps);
    const outCurve = new Float32Array(steps);
    for (let i = 0; i < steps; i++) {
      const t = i / (steps - 1);
      if (curve === 'linear') {
        inCurve[i] = t;
        outCurve[i] = 1 - t;
      } else if (curve === 's-curve') {
        const s = t * t * (3 - 2 * t);
        inCurve[i] = Math.sin(s * Math.PI * 0.5);
        outCurve[i] = Math.cos(s * Math.PI * 0.5);
      } else {
        inCurve[i] = Math.sin(t * Math.PI * 0.5);
        outCurve[i] = Math.cos(t * Math.PI * 0.5);
      }
    }
    try {
      inGain.setValueCurveAtTime(inCurve, now, durationSec);
      outGain.setValueCurveAtTime(outCurve, now, durationSec);
    } catch {
      inGain.cancelScheduledValues(0);
      outGain.cancelScheduledValues(0);
      inGain.setValueAtTime(0.001, now);
      inGain.linearRampToValueAtTime(1, now + durationSec);
      outGain.setValueAtTime(1, now);
      outGain.linearRampToValueAtTime(0, now + durationSec);
    }

    if (preset === 'rise') {
      inFilter.type = 'lowpass';
      inFilter.Q.value = 1.1;
      inFilter.frequency.setValueAtTime(350, now);
      inFilter.frequency.exponentialRampToValueAtTime(18000, now + durationSec);
      outFilter.type = 'highpass';
      outFilter.frequency.setValueAtTime(60, now);
      outFilter.frequency.exponentialRampToValueAtTime(1400, now + durationSec);
    } else {
      inFilter.type = 'allpass';
      outFilter.type = 'allpass';
    }
  }

  /**
   * Мгновенный отклик на клик «Вперёд/Назад/Трек»:
   * мягко приглушает текущую деку на время резолва и буферизации нового трека,
   * чтобы не было ни тишины, ни задержки реакции.
   */
  prepareManualSkipDuck(outgoingDeck: 'A' | 'B', style: string) {
    if (!this.ctx || !this.deckGainA || !this.deckGainB || !this.deckFilterA || !this.deckFilterB) return;
    if (style === 'off') return;
    const now = this.ctx.currentTime;
    const outGain = (outgoingDeck === 'A' ? this.deckGainA : this.deckGainB).gain;
    const outFilter = outgoingDeck === 'A' ? this.deckFilterA : this.deckFilterB;

    outGain.cancelScheduledValues(now);
    outFilter.frequency.cancelScheduledValues(now);

    if (style === 'clean-whip') {
      outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
      outGain.linearRampToValueAtTime(0.28, now + 0.08);
      return;
    }

    outFilter.type = 'lowpass';
    outFilter.Q.value = 0.75;
    outFilter.frequency.setValueAtTime(Math.min(16000, Math.max(400, outFilter.frequency.value || 16000)), now);
    outFilter.frequency.exponentialRampToValueAtTime(2600, now + 0.12);

    outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
    outGain.linearRampToValueAtTime(0.45, now + 0.12);
  }

  restoreDeck(deck: 'A' | 'B') {
    if (!this.ctx || !this.deckGainA || !this.deckGainB || !this.deckFilterA || !this.deckFilterB) return;
    const now = this.ctx.currentTime;
    const g = (deck === 'A' ? this.deckGainA : this.deckGainB).gain;
    const f = deck === 'A' ? this.deckFilterA : this.deckFilterB;
    g.cancelScheduledValues(now);
    f.frequency.cancelScheduledValues(now);
    f.type = 'allpass';
    g.setTargetAtTime(1, now, 0.06);
  }

  holdManualSkipIncoming(incomingDeck: 'A' | 'B') {
    if (!this.ctx || !this.deckGainA || !this.deckGainB || !this.deckFilterA || !this.deckFilterB) return;
    const now = this.ctx.currentTime;
    const inGain = (incomingDeck === 'A' ? this.deckGainA : this.deckGainB).gain;
    const inFilter = incomingDeck === 'A' ? this.deckFilterA : this.deckFilterB;
    inGain.cancelScheduledValues(now);
    inFilter.frequency.cancelScheduledValues(now);
    inFilter.type = 'allpass';
    inGain.setValueAtTime(0, now);
  }

  private triggerWhoosh(durationSec = 0.4) {
    if (!this.ctx || !this.analyser) return;
    try {
      const sr = this.ctx.sampleRate;
      const len = Math.max(1, Math.floor(sr * durationSec));
      const buf = this.ctx.createBuffer(1, len, sr);
      const data = buf.getChannelData(0);
      let last = 0;
      for (let i = 0; i < len; i++) {
        const white = Math.random() * 2 - 1;
        last = (last + 0.14 * white) / 1.14;
        data[i] = last * 3.0;
      }

      const src = this.ctx.createBufferSource();
      src.buffer = buf;

      const bp = this.ctx.createBiquadFilter();
      bp.type = 'bandpass';
      bp.Q.value = 1.1;

      const g = this.ctx.createGain();
      const now = this.ctx.currentTime;
      bp.frequency.setValueAtTime(240, now);
      bp.frequency.exponentialRampToValueAtTime(4500, now + durationSec * 0.42);
      bp.frequency.exponentialRampToValueAtTime(650, now + durationSec);

      g.gain.setValueAtTime(0.0001, now);
      g.gain.linearRampToValueAtTime(0.28, now + durationSec * 0.36);
      g.gain.exponentialRampToValueAtTime(0.0001, now + durationSec);

      src.connect(bp);
      bp.connect(g);
      g.connect(this.analyser);
      src.start(now);
      src.stop(now + durationSec + 0.02);
    } catch {
      // ignore
    }
  }

  private triggerVinylBrake(el: HTMLAudioElement | undefined | null, durationSec = 0.26) {
    if (!el) return;
    this.cancelBeatmatch(el);
    this.setPreservesPitch(el, false);
    const startMs = performance.now();
    const durMs = durationSec * 1000;
    const step = (now: number) => {
      const t = Math.min(1, (now - startMs) / durMs);
      const eased = t * t;
      el.playbackRate = Math.max(0.25, 1 - 0.75 * eased);
      if (t >= 1) {
        this.beatmatchRaf.delete(el);
        return;
      }
      this.beatmatchRaf.set(el, requestAnimationFrame(step));
    };
    this.beatmatchRaf.set(el, requestAnimationFrame(step));
  }

  /**
   * Бесшовная передача эстафеты в момент реального старта входящего трека (playing):
   * уходящий трек плавно уходит в 0, входящий чисто открывается без наложения баса и без паузы.
   */
  handoffManualSkip(
    incomingDeck: 'A' | 'B',
    style: string,
    outgoingEl?: HTMLAudioElement | null
  ): number {
    if (!this.ctx || !this.deckGainA || !this.deckGainB || !this.deckFilterA || !this.deckFilterB) {
      return 0;
    }
    if (style === 'off') {
      this.crossfadeTo(incomingDeck, 0, 'fade', 'equal-power');
      return 0;
    }

    const now = this.ctx.currentTime;
    const inGain = (incomingDeck === 'A' ? this.deckGainA : this.deckGainB).gain;
    const outGain = (incomingDeck === 'A' ? this.deckGainB : this.deckGainA).gain;
    const inFilter = incomingDeck === 'A' ? this.deckFilterA : this.deckFilterB;
    const outFilter = incomingDeck === 'A' ? this.deckFilterB : this.deckFilterA;

    inGain.cancelScheduledValues(now);
    outGain.cancelScheduledValues(now);
    inFilter.frequency.cancelScheduledValues(now);
    inFilter.gain.cancelScheduledValues(now);
    outFilter.frequency.cancelScheduledValues(now);
    outFilter.gain.cancelScheduledValues(now);

    if (style === 'vinyl-brake') {
      inFilter.type = 'allpass';
      outFilter.type = 'lowpass';
      outFilter.frequency.setValueAtTime(Math.min(16000, Math.max(400, outFilter.frequency.value || 4000)), now);
      outFilter.frequency.exponentialRampToValueAtTime(240, now + 0.25);
      outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
      outGain.linearRampToValueAtTime(0, now + 0.26);
      this.triggerVinylBrake(outgoingEl, 0.25);

      inGain.setValueAtTime(0, now);
      inGain.setValueAtTime(0, now + 0.08);
      inGain.linearRampToValueAtTime(1, now + 0.22);
      return 0.28;
    }

    if (style === 'tape-whoosh') {
      inFilter.type = 'allpass';
      outFilter.type = 'highpass';
      outFilter.Q.value = 1.2;
      outFilter.frequency.setValueAtTime(80, now);
      outFilter.frequency.exponentialRampToValueAtTime(2200, now + 0.26);
      outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
      outGain.linearRampToValueAtTime(0, now + 0.28);
      this.triggerWhoosh(0.4);

      inGain.setValueAtTime(0, now);
      inGain.setValueAtTime(0, now + 0.12);
      inGain.linearRampToValueAtTime(1, now + 0.3);
      return 0.34;
    }

    if (style === 'clean-whip') {
      inFilter.type = 'allpass';
      outFilter.type = 'allpass';
      outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
      outGain.linearRampToValueAtTime(0, now + 0.07);
      inGain.setValueAtTime(0, now);
      inGain.setValueAtTime(0, now + 0.1);
      inGain.linearRampToValueAtTime(1, now + 0.16);
      return 0.18;
    }

    if (style === 'echo-tail') {
      inFilter.type = 'allpass';
      outFilter.type = 'lowpass';
      outFilter.Q.value = 1.1;
      outFilter.frequency.setValueAtTime(Math.min(16000, Math.max(600, outFilter.frequency.value || 4500)), now);
      outFilter.frequency.exponentialRampToValueAtTime(360, now + 0.38);
      outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
      outGain.linearRampToValueAtTime(0, now + 0.42);

      if (this.wetGain) {
        const baseWet = this.wetGain.gain.value;
        this.wetGain.gain.cancelScheduledValues(now);
        this.wetGain.gain.setValueAtTime(Math.max(baseWet, 0.68), now);
        this.wetGain.gain.linearRampToValueAtTime(baseWet, now + 0.95);
      }

      inGain.setValueAtTime(0, now);
      inGain.setValueAtTime(0, now + 0.18);
      inGain.linearRampToValueAtTime(1, now + 0.4);
      return 0.46;
    }

    // По умолчанию: filter-cut — выраженный резонансный DJ-срез + чистый вход
    inFilter.type = 'highpass';
    inFilter.Q.value = 0.8;
    inFilter.frequency.setValueAtTime(220, now);
    inFilter.frequency.exponentialRampToValueAtTime(22, now + 0.24);

    outFilter.type = 'lowpass';
    outFilter.Q.value = 2.4;
    outFilter.frequency.setValueAtTime(Math.min(16000, Math.max(400, outFilter.frequency.value || 4500)), now);
    outFilter.frequency.exponentialRampToValueAtTime(140, now + 0.36);
    outGain.setValueAtTime(Math.max(outGain.value, 0.001), now);
    outGain.linearRampToValueAtTime(0, now + 0.38);

    inGain.setValueAtTime(0, now);
    inGain.setValueAtTime(0, now + 0.12);
    inGain.linearRampToValueAtTime(1, now + 0.34);
    return 0.42;
  }

  async resume() {
    if (this.ctx && this.ctx.state === 'suspended') {
      try {
        await this.ctx.resume();
      } catch {
        // ignore
      }
    }
  }

  applyEffects(effects: AudioEffectsConfig) {
    if (!this.ctx) return;
    const now = this.ctx.currentTime;

    if (this.preampGain) {
      const clampedDb = Math.max(-24, Math.min(6, effects.preamp_db ?? 0));
      const linear = effects.eq_enabled ? Math.pow(10, clampedDb / 20) : 1.0;
      this.preampGain.gain.setTargetAtTime(linear, now, 0.03);
    }

    this.filters.forEach((f, idx) => {
      const targetGain = effects.eq_enabled ? (effects.bands[idx] ?? 0) : 0;
      f.gain.setTargetAtTime(targetGain, now, 0.03);
    });

    if (this.bassFilter) {
      const bassDb = (effects.bass_boost / 100) * 10.5;
      this.bassFilter.gain.setTargetAtTime(bassDb, now, 0.03);
    }

    if (this.compressor && this.makeupGain) {
      if (effects.normalize_volume) {
        this.compressor.threshold.setTargetAtTime(-18, now, 0.04);
        this.compressor.ratio.setTargetAtTime(4.5, now, 0.04);
        this.makeupGain.gain.setTargetAtTime(1.18, now, 0.04);
      } else {
        this.compressor.threshold.setTargetAtTime(0, now, 0.04);
        this.compressor.ratio.setTargetAtTime(1, now, 0.04);
        this.makeupGain.gain.setTargetAtTime(1.0, now, 0.04);
      }
    }

    if (this.wetGain && this.dryGain) {
      if (effects.spatial_audio) {
        this.dryGain.gain.setTargetAtTime(0.0, now, 0.04);
        this.wetGain.gain.setTargetAtTime(1.0, now, 0.04);
      } else {
        this.dryGain.gain.setTargetAtTime(1.0, now, 0.04);
        this.wetGain.gain.setTargetAtTime(0.0, now, 0.04);
      }
    }
  }

  async setOutputDevice(deviceId: string): Promise<boolean> {
    this.pendingSinkId = deviceId || 'default';
    if (!this.ctx) return true;
    if (!('setSinkId' in this.ctx)) return false;
    const ctxWithSink = this.ctx as AudioContext & { setSinkId: (id: string) => Promise<void> };
    await ctxWithSink.setSinkId(this.pendingSinkId === 'default' ? '' : this.pendingSinkId);
    return true;
  }

  getFrequencyData(): Uint8Array | null {
    if (!this.analyser || !this.freqBuf) return null;
    this.analyser.getByteFrequencyData(this.freqBuf);
    return this.freqBuf;
  }
}

export const audioEngine = new AudioEngine();

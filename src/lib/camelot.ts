import type { Track } from './types';

export interface TrackAudioFeatures {
  bpm: number;
  energy: number; // 0.0 .. 1.0
  camelot: string; // e.g. "8A", "11B"
  keyName: string; // e.g. "Am", "F#m"
  analyzed: boolean;
}

const STORAGE_KEY = 'flowly_audio_features_v1';

// Krumhansl-Schmuckler профили тональностей (C .. B)
const MAJOR_PROFILE = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE = [6.33, 2.68, 3.52, 5.38, 2.6, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

// Маппинг 12 питч-классов (0=C .. 11=B) в Колесо Камелота
const MAJOR_CAMELOT: { code: string; name: string }[] = [
  { code: '8B', name: 'C' },
  { code: '3B', name: 'Db' },
  { code: '10B', name: 'D' },
  { code: '5B', name: 'Eb' },
  { code: '12B', name: 'E' },
  { code: '7B', name: 'F' },
  { code: '2B', name: 'F#' },
  { code: '9B', name: 'G' },
  { code: '4B', name: 'Ab' },
  { code: '11B', name: 'A' },
  { code: '6B', name: 'Bb' },
  { code: '1B', name: 'B' },
];

const MINOR_CAMELOT: { code: string; name: string }[] = [
  { code: '5A', name: 'Cm' },
  { code: '12A', name: 'C#m' },
  { code: '7A', name: 'Dm' },
  { code: '2A', name: 'Ebm' },
  { code: '9A', name: 'Em' },
  { code: '4A', name: 'Fm' },
  { code: '11A', name: 'F#m' },
  { code: '6A', name: 'Gm' },
  { code: '1A', name: 'G#m' },
  { code: '8A', name: 'Am' },
  { code: '3A', name: 'Bbm' },
  { code: '10A', name: 'Bm' },
];

function loadCache(): Record<string, TrackAudioFeatures> {
  if (typeof localStorage === 'undefined') return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Record<string, TrackAudioFeatures>) : {};
  } catch {
    return {};
  }
}

let featureCache: Record<string, TrackAudioFeatures> = loadCache();

function saveCache() {
  if (typeof localStorage === 'undefined') return;
  try {
    const entries = Object.entries(featureCache);
    if (entries.length > 600) {
      featureCache = Object.fromEntries(entries.slice(entries.length - 500));
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(featureCache));
  } catch {
    // ignore quota errors
  }
}

function hashTrackId(id: string): number {
  let h = 2166136261;
  const str = id == null ? '' : String(id);
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function getTrackAudioFeatures(track: Track): TrackAudioFeatures {
  // Важно: h может оказаться > 2^31 — тогда h >> 4 уходит в минус,
  // и pitch становится отрицательным. Поэтому cam берём с фоллбэком.
  const id = track?.id != null ? String(track.id) : '';
  const cached = featureCache[id];
  if (cached) return cached;

  const g = (track?.genre ?? '').toLowerCase();
  const h = hashTrackId(id);

  let baseBpm = 118;
  let baseEnergy = 0.62;

  if (/dnb|drum|jungle/.test(g)) {
    baseBpm = 168 + (h % 10);
    baseEnergy = 0.88;
  } else if (/phonk|techno|hard|metal|punk/.test(g)) {
    baseBpm = 132 + (h % 18);
    baseEnergy = 0.84;
  } else if (/rap|hip|trap|drill|rusrap/.test(g)) {
    baseBpm = 122 + (h % 24);
    baseEnergy = 0.72;
  } else if (/house|electr|dance|edm|synth/.test(g)) {
    baseBpm = 120 + (h % 12);
    baseEnergy = 0.78;
  } else if (/rock|alt|grunge|indie/.test(g)) {
    baseBpm = 108 + (h % 26);
    baseEnergy = 0.7;
  } else if (/lo-?fi|ambient|chill|jazz|acoustic|relax/.test(g)) {
    baseBpm = 76 + (h % 20);
    baseEnergy = 0.36;
  } else {
    baseBpm = 98 + (h % 34);
    baseEnergy = 0.56;
  }

  const energyJitter = ((h % 17) - 8) * 0.015;
  const energy = Math.max(0.15, Math.min(0.98, Number((baseEnergy + energyJitter).toFixed(2))));

  const isMinor = h % 3 !== 0;
  const pitch = Math.abs((h >> 4) % 12);
  const cam = (isMinor ? MINOR_CAMELOT[pitch] : MAJOR_CAMELOT[pitch]) ?? MINOR_CAMELOT[0];

  return {
    bpm: baseBpm,
    energy,
    camelot: cam.code,
    keyName: cam.name,
    analyzed: false,
  };
}

export function parseCamelot(code: string): { num: number; mode: 'A' | 'B' } {
  const mode = code.endsWith('B') ? 'B' : 'A';
  const num = Math.max(1, Math.min(12, parseInt(code, 10) || 1));
  return { num, mode };
}

export function camelotCompatibility(aCode: string, bCode: string): number {
  const a = parseCamelot(aCode);
  const b = parseCamelot(bCode);

  if (a.num === b.num && a.mode === b.mode) return 1.0; // Идеальное совпадение
  if (a.num === b.num && a.mode !== b.mode) return 0.94; // Параллельные мажор/минор (8A <-> 8B)

  const diff = Math.min((a.num - b.num + 12) % 12, (b.num - a.num + 12) % 12);
  if (a.mode === b.mode && diff === 1) return 0.9; // Соседние по колесу (±1)
  if (a.mode === b.mode && diff === 2) return 0.65; // Энергетический скачок (+2)
  if (diff === 1 && a.mode !== b.mode) return 0.55;
  return 0.2;
}

function bpmProximity(bpmA: number, bpmB: number): number {
  const direct = Math.abs(bpmA - bpmB);
  const half = Math.abs(bpmA - bpmB * 2);
  const dbl = Math.abs(bpmA * 2 - bpmB);
  const bestDiff = Math.min(direct, half, dbl);
  return Math.max(0, 1 - bestDiff / 32);
}

export function pickSmartNextTrack(
  current: Track,
  queue: Track[],
  playedIds: Set<string>,
  opts: { smartShuffle: boolean; harmonicMix: boolean }
): Track | null {
  const candidates = queue.filter((t) => t.id !== current.id && !playedIds.has(t.id));
  const pool = candidates.length > 0 ? candidates : queue.filter((t) => t.id !== current.id);
  if (pool.length === 0) return null;

  const curFeat = getTrackAudioFeatures(current);

  const scored = pool.map((cand) => {
    const f = getTrackAudioFeatures(cand);
    const camScore = camelotCompatibility(curFeat.camelot, f.camelot);
    const bpmScore = bpmProximity(curFeat.bpm, f.bpm);
    const energyScore = Math.max(0, 1 - Math.abs(curFeat.energy - f.energy) / 0.45);

    let total = 0;
    if (opts.harmonicMix && opts.smartShuffle) {
      total = camScore * 0.45 + bpmScore * 0.35 + energyScore * 0.2;
    } else if (opts.harmonicMix) {
      total = camScore * 0.72 + bpmScore * 0.28;
    } else {
      total = bpmScore * 0.55 + energyScore * 0.45;
    }
    return { track: cand, total };
  });

  scored.sort((a, b) => b.total - a.total);
  const topN = scored.slice(0, Math.min(2, scored.length));
  return topN[Math.floor(Math.random() * topN.length)].track;
}

export function getHarmonicCrossfadeSec(
  current: Track,
  next: Track,
  baseSec: number,
  harmonicMix: boolean
): number {
  if (!harmonicMix || baseSec <= 0) return baseSec;
  const fA = getTrackAudioFeatures(current);
  const fB = getTrackAudioFeatures(next);
  const compat = camelotCompatibility(fA.camelot, fB.camelot);
  if (compat >= 0.88) return Math.min(12, baseSec + 2);
  if (compat <= 0.3) return Math.max(2, baseSec - 2);
  return baseSec;
}

function correlateKey(chroma: Float64Array, profile: number[], shift: number): number {
  let num = 0;
  let denA = 0;
  let denB = 0;
  for (let i = 0; i < 12; i++) {
    const a = chroma[(i + shift) % 12];
    const b = profile[i];
    num += a * b;
    denA += a * a;
    denB += b * b;
  }
  const den = Math.sqrt(denA * denB);
  return den > 0 ? num / den : 0;
}

/**
 * Реальный анализатор спектра в реальном времени (Хромограмма + RMS Энергия + Onset BPM).
 */
export class LiveAudioAnalyzer {
  private trackId: string | null = null;
  private chroma = new Float64Array(12);
  private energySum = 0;
  private frames = 0;
  private prevBass = 0;
  private peakTimestamps: number[] = [];
  private lastPeakAt = 0;

  reset(trackId: string | null) {
    this.trackId = trackId;
    this.chroma.fill(0);
    this.energySum = 0;
    this.frames = 0;
    this.prevBass = 0;
    this.peakTimestamps = [];
    this.lastPeakAt = 0;
  }

  feed(track: Track, freq: Uint8Array, sampleRate = 48000): TrackAudioFeatures | null {
    if (this.trackId !== track.id) {
      this.reset(track.id);
    }
    if (featureCache[track.id]?.analyzed) {
      return featureCache[track.id];
    }

    const binCount = freq.length;
    const nyquist = sampleRate / 2;
    const hzPerBin = nyquist / binCount;

    let frameEnergy = 0;
    let bassEnergy = 0;
    let bassBins = 0;

    for (let i = 1; i < binCount; i++) {
      const mag = freq[i] / 255;
      frameEnergy += mag * mag;
      const hz = i * hzPerBin;
      if (hz >= 45 && hz <= 160) {
        bassEnergy += mag;
        bassBins++;
      }
      if (hz >= 65 && hz <= 2000) {
        const midi = 69 + 12 * Math.log2(hz / 440);
        const pitchClass = ((Math.round(midi) % 12) + 12) % 12;
        this.chroma[pitchClass] += mag;
      }
    }

    const rms = Math.sqrt(frameEnergy / Math.max(1, binCount));
    this.energySum += Math.min(1, rms * 2.1);
    this.frames += 1;

    const avgBass = bassBins > 0 ? bassEnergy / bassBins : 0;
    const now = performance.now();
    if (avgBass > 0.42 && avgBass - this.prevBass > 0.08 && now - this.lastPeakAt > 280) {
      this.peakTimestamps.push(now);
      this.lastPeakAt = now;
    }
    this.prevBass = avgBass;

    // После ~100 кадров (~4-5 сек) сохраняем реальные показатели трека
    if (this.frames === 110) {
      let bestScore = -1;
      let bestCam = MINOR_CAMELOT[0];
      for (let shift = 0; shift < 12; shift++) {
        const maj = correlateKey(this.chroma, MAJOR_PROFILE, shift);
        if (maj > bestScore) {
          bestScore = maj;
          bestCam = MAJOR_CAMELOT[shift];
        }
        const min = correlateKey(this.chroma, MINOR_PROFILE, shift);
        if (min > bestScore) {
          bestScore = min;
          bestCam = MINOR_CAMELOT[shift];
        }
      }

      let measuredBpm = getTrackAudioFeatures(track).bpm;
      if (this.peakTimestamps.length >= 4) {
        const intervals: number[] = [];
        for (let i = 1; i < this.peakTimestamps.length; i++) {
          const dt = this.peakTimestamps[i] - this.peakTimestamps[i - 1];
          if (dt >= 300 && dt <= 1050) intervals.push(dt);
        }
        if (intervals.length >= 3) {
          intervals.sort((a, b) => a - b);
          const medianMs = intervals[Math.floor(intervals.length / 2)];
          let bpm = Math.round(60000 / medianMs);
          while (bpm < 72) bpm *= 2;
          while (bpm > 175) bpm = Math.round(bpm / 2);
          measuredBpm = bpm;
        }
      }

      const measured: TrackAudioFeatures = {
        bpm: measuredBpm,
        energy: Number(Math.max(0.15, Math.min(0.98, this.energySum / this.frames)).toFixed(2)),
        camelot: bestCam.code,
        keyName: bestCam.name,
        analyzed: true,
      };
      featureCache[track.id] = measured;
      saveCache();
      return measured;
    }

    return null;
  }
}

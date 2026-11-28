<script lang="ts">
  // Пиксель-перфект порт вайб-анимации официального Яндекса (vibe-animation-worker):
  // три шумовых круга, искры по краю, цвета из обложки, отклик на звук и на лайк.
  // Формулы перенесены из CPU-порта yamusic-rs (vibe.rs); рендер — WebGL фрагментный
  // шейдер (у оригинала тоже WebGL). Полосы звука: 0–250 / 500–2000 / 2000–4000 Гц.

  import { onMount } from 'svelte';
  import { audioEngine } from '../audioEngine';

  let {
    color,
    playing,
    likePulse = 0,
  }: { color: string; playing: boolean; likePulse?: number } = $props();

  // ---------- палитра из обложки (Palette::from_rgb) ----------

  interface Hsl {
    h: number; // 0–360
    s: number; // 0–1
    l: number; // 0–1
  }
  interface Palette {
    top: Hsl;
    middle: Hsl;
    bottom: Hsl;
  }

  function hexToRgb(hex: string): [number, number, number] | null {
    const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
    if (!m) return null;
    const n = parseInt(m[1], 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }

  // h: 0–360 (до градуса), s/l: проценты с одним знаком — так считает оригинал
  function rgbToHsl(c: [number, number, number]): [number, number, number] {
    const [r, g, b] = c.map((v) => v / 255);
    const min = Math.min(r, g, b);
    const max = Math.max(r, g, b);
    const d = max - min;
    const l = (min + max) / 2;
    let h = 0;
    if (d !== 0) {
      if (max === r) h = ((g - b) / d) % 6;
      else if (max === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      h = Math.round(h * 60);
      if (h < 0) h += 360;
    }
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
    const tenth = (v: number) => Math.round(v * 1000) / 10;
    return [h, tenth(s), tenth(l)];
  }

  // Чем меньше круг, тем он ярче и насыщеннее: +15/+10, +15/+5, без изменений
  function paletteFromRgb(c: [number, number, number]): Palette {
    const [h, s, l] = rgbToHsl(c);
    const mk = (ds: number, dl: number): Hsl => ({
      h: Math.min(360, Math.max(0, h)),
      s: Math.min(1, Math.max(0, (s + ds) / 100)),
      l: Math.min(1, Math.max(0, (l + dl) / 100)),
    });
    return { top: mk(15, 10), middle: mk(15, 5), bottom: mk(0, 0) };
  }

  // Насколько пятно больше обложки: 1.0 — края впритык, 1.5 — кольца и лучи
  // заметно выходят за углы, 2.0+ — пятно заполняет пол-экрана
  const COVER_FIT = 1.5;

  const DEFAULT_PALETTE: Palette = {
    top: { h: 50, s: 1, l: 0.5 },
    middle: { h: 330, s: 1, l: 0.5 },
    bottom: { h: 300, s: 1, l: 0.5 },
  };

  // Формула воркера: l - a * clamp(min(k-3, 9-k), -1, 1), a = s * min(l, 1-l)
  function hslToRgb(h: number, s: number, l: number): [number, number, number] {
    const f = (n: number) => {
      const k = (n + h / 30) % 12;
      const a = s * Math.min(l, 1 - l);
      const cl = Math.max(-1, Math.min(1, Math.min(k - 3, 9 - k)));
      return l - a * cl;
    };
    return [f(0), f(8), f(4)];
  }

  // Уровень полосы растёт сразу, а спадает не быстрее 0,02 за кадр
  function audioParam(prev: number, nw: number): number {
    const s = Math.min(1, Math.max(0, prev));
    const n = Math.min(1, Math.max(0, nw));
    if (n > s) return n;
    if (n < s) return s - Math.min(s - n, 0.02);
    return s;
  }

  const SCALE = 0.35;

  function makeRng(): { unit(): number; range(min: number, max: number): number } {
    let state = (Date.now() | 1) >>> 0;
    const unit = () => {
      state ^= state << 13;
      state ^= state >>> 7;
      state ^= state << 17;
      return (state >>> 11) / 2 ** 53;
    };
    return { unit, range: (min, max) => Math.floor(unit() * (Math.floor(max) - min + 1)) + min };
  }

  // Плавное значение воркера: каждый кадр подходит к цели на долю,
  // растущую со временем. Когда время перехода вышло — встаёт точно на цель.
  class Tween {
    cur: number;
    target: number;
    elapsed = 0;
    duration: number;
    constructor(v: number, duration: number) {
      this.cur = v;
      this.target = v;
      this.duration = duration;
    }
    update(target: number) {
      this.target = target;
      this.elapsed = 0;
    }
    next(dt: number) {
      const k = Math.min(1, this.elapsed / this.duration);
      this.elapsed += dt;
      this.cur += (this.target - this.cur) * k;
    }
    done() {
      return this.cur === this.target;
    }
  }

  // Цвет одного круга: три канала идут к цели за 3 секунды
  class Tint {
    r: Tween;
    g: Tween;
    b: Tween;
    constructor(h: number) {
      const [r, g, b] = hslToRgb(h, 1, 0.5);
      this.r = new Tween(r, 3000);
      this.g = new Tween(g, 3000);
      this.b = new Tween(b, 3000);
    }
    update(c: Hsl) {
      const [r, g, b] = hslToRgb(c.h, c.s, c.l);
      this.r.update(r);
      this.g.update(g);
      this.b.update(b);
    }
    next(dt: number) {
      this.r.next(dt);
      this.g.next(dt);
      this.b.next(dt);
    }
    done() {
      return this.r.done() && this.g.done() && this.b.done();
    }
    value(): [number, number, number] {
      return [this.r.cur, this.g.cur, this.b.cur];
    }
  }

  // Шесть цветов шейдера: начало и конец градиента каждого круга
  class Colors {
    top: [Tint, Tint];
    middle: [Tint, Tint];
    bottom: [Tint, Tint];
    constructor(rng: { range(min: number, max: number): number }) {
      // Colors::new: collectionHue по умолчанию 10
      const shifted = (hue: number) => (hue + 280) % 360;
      const wrap = (t: number, e: number) => (e >= 280 && e < 360 ? t % 360 : t);
      const e = shifted(10);
      const i = wrap(e + rng.range(30, 40), e);
      this.top = [new Tint(50), new Tint(50)];
      this.middle = [new Tint(300), new Tint(320)];
      this.bottom = [new Tint(e), new Tint(i)];
    }
    update(p: Palette) {
      this.top.forEach((t) => t.update(p.top));
      this.middle.forEach((t) => t.update(p.middle));
      this.bottom.forEach((t) => t.update(p.bottom));
    }
    next(dt: number) {
      for (const t of [...this.top, ...this.middle, ...this.bottom]) t.next(dt);
    }
    done() {
      return [...this.top, ...this.middle, ...this.bottom].every((t) => t.done());
    }
    // Порядок vColor: начала нижнего, среднего и верхнего круга, затем их концы
    value(): number[] {
      const b0 = this.bottom[0].value();
      const m0 = this.middle[0].value();
      const t0 = this.top[0].value();
      const b1 = this.bottom[1].value();
      const m1 = this.middle[1].value();
      const t1 = this.top[1].value();
      return [...b0, ...m0, ...t0, ...b1, ...m1, ...t1];
    }
  }

  // Состояние воркера между кадрами (State из vibe.rs)
  class VibeState {
    energy = new Tween(0.2, 1000);
    time: number;
    colors: Colors;
    audio: [number, number, number] = [0, 0, 0];
    audioRatio = new Tween(0, 1000);
    freq: [number, number, number] = [0, 0, 0];
    react: [Tween, Tween, Tween] = [new Tween(0, 600), new Tween(0, 600), new Tween(0, 600)];
    timers: { at: number; circle: number; value: number }[] = [];
    clock = 0;

    constructor(rng: { unit(): number; range(min: number, max: number): number }) {
      this.time = Math.floor(3600 * rng.unit());
      this.colors = new Colors(rng);
    }

    // playAnimation: энергия по умолчанию 0,6, в шейдер идёт 0.4 * (energy + 1)
    play() {
      this.audioRatio.update(1);
      this.energy.update(0.4 * (0.6 + 1.0));
    }

    idle() {
      this.energy.update(0.2);
      this.audioRatio.update(0);
    }

    // Лайк: круги вспыхивают по очереди и гаснут через 0,85–1,05 секунды
    like() {
      this.react[0].update(0.7);
      for (const [ms, circle, value] of [
        [100, 1, 0.7],
        [150, 2, 0.7],
        [850, 0, 0],
        [950, 1, 0],
        [1050, 2, 0],
      ] as const) {
        this.timers.push({ at: this.clock + ms, circle, value });
      }
    }

    // Сработать таймерам, чьё время пришло (у оригинала setTimeout между кадрами)
    fireTimers() {
      const due = this.timers.filter((t) => t.at <= this.clock);
      this.timers = this.timers.filter((t) => t.at > this.clock);
      for (const t of due) this.react[t.circle].update(t.value);
    }

    update(dt: number) {
      this.energy.next(dt);
      this.colors.next(dt);
      for (const t of this.react) t.next(dt);
      this.time = (this.time + (this.energy.cur * dt) / 1000) % 86400;
      this.audioRatio.next(dt);
      for (let i = 0; i < 3; i++) {
        this.audio[i] = audioParam(this.audio[i], this.freq[i]) * this.audioRatio.cur;
      }
    }

    // Все переходы у цели, звук и вспышки погасли: дальше меняется только время
    resting() {
      return (
        this.energy.done() &&
        this.audioRatio.done() &&
        this.react.every((t) => t.done()) &&
        this.colors.done() &&
        this.timers.length === 0 &&
        this.audio.every((v) => v === 0)
      );
    }
  }

  const state = new VibeState(makeRng());

  let canvasEl: HTMLCanvasElement;
  let playingFlag = false;
  let shownAt = performance.now();

  // Полосы звука: средние уровни трёх полос, как getAverageFrequencies оригинала.
  // Ширина полосы — rate / binCount (вдвое шире настоящей), так и оставлено:
  // иначе отклик был бы другим.
  function bands(freq: Uint8Array | null, rate: number | null): [number, number, number] {
    if (!freq || !rate || !playingFlag) return [0, 0, 0];
    const binHz = rate / freq.length;
    const avg = (lo: number, hi: number) => {
      const a = Math.floor(lo / binHz);
      const b = Math.floor(hi / binHz);
      let sum = 0;
      for (let i = a; i <= b; i++) sum += (freq[i] ?? 0) / 255;
      return sum / Math.max(1, b - a + 1);
    };
    return [avg(0, 250), avg(500, 2000), avg(2000, 4000)];
  }

  // Кривая времени CSS: cubic-bezier(x1, y1, x2, y2) — для проявления при показе
  function cubicBezier(x1: number, y1: number, x2: number, y2: number, x: number): number {
    const t = Math.min(1, Math.max(0, x));
    const at = (a: number, b: number, tt: number) =>
      ((1 - 3 * b + 3 * a) * tt + (3 * b - 6 * a)) * tt * tt + 3 * a * tt;
    let lo = 0;
    let hi = 1;
    let tt = t;
    for (let i = 0; i < 24; i++) {
      const v = at(x1, x2, tt);
      if (Math.abs(v - t) < 1e-5) break;
      if (v < t) lo = tt;
      else hi = tt;
      tt = (lo + hi) / 2;
    }
    return at(y1, y2, tt);
  }

  const FRAG = `
precision highp float;
uniform float uTime;
uniform float uOpacity;
uniform float uCircles;
uniform vec2 uCenter;
uniform float uUnit;
uniform float uRadius;
uniform vec3 uColors[6];
uniform vec3 uAudio;
uniform vec3 uReact;

const float PI_GL = 3.1415;
const float CIRCLE_WIDTH_BASE = 0.8;
const float CIRCLE_WIDTH_STEP = 0.2;
const float SPARK_STRENGTH_BASE = 1.0;
const float SPARK_STRENGTH_STEP = 0.3;
const float CIRCLE_OFFSET_BASE = 0.0;
const float CIRCLE_OFFSET_STEP = 1.57;
// Отношение радиусов слоёв оригинала: 0.8/0.95 и 0.65/0.95
const float R1K = 0.15 / 0.95;
const float R2K = 0.30 / 0.95;
const float W1K = 0.20 / 0.80;
const float W2K = 0.40 / 0.80;
const vec3 ROT0 = vec3(-0.3, 0.3, 0.2);
const vec3 ROT1 = vec3(-0.3, -0.3, -0.2);
const vec3 ROT2 = vec3(-0.3, -0.3, 0.2);

// smoothstep GLSL: края могут идти в обратном порядке — шейдер этим пользуется
float ss(float e0, float e1, float x) {
  float t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}
float tri(float x) { return abs(fract(x) - 0.5); }

vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec4 mod289(vec4 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec4 permute(vec4 x) { return mod289(((x * 34.0) + 1.0) * x); }
vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }

// Симплекс-шум Ashima из шейдера оригинала (permute c +1.0, как в vibe.rs)
float snoise3(vec3 v) {
  const vec2 C = vec2(1.0 / 6.0, 1.0 / 3.0);
  const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);

  vec3 i = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);

  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);

  vec3 x1 = x0 - i1 + 1.0 * C.xxx;
  vec3 x2 = x0 - i2 + 2.0 * C.xxx;
  vec3 x3 = x0 - 1.0 + 3.0 * C.xxx;

  i = mod289(i);
  vec4 p = permute(permute(permute(
        i.z + vec4(0.0, i1.z, i2.z, 1.0))
      + i.y + vec4(0.0, i1.y, i2.y, 1.0))
      + i.x + vec4(0.0, i1.x, i2.x, 1.0));

  float n_ = 0.142857142857;
  vec3 ns = n_ * D.wyz - D.xzx;

  vec4 j = p - 49.0 * floor(p * ns.z * ns.z);

  vec4 x_ = floor(j * ns.z);
  vec4 y_ = floor(j - 7.0 * x_);

  vec4 x = x_ * ns.x + ns.yyyy;
  vec4 y = y_ * ns.x + ns.yyyy;
  vec4 h = 1.0 - abs(x) - abs(y);

  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);

  vec4 s0 = floor(b0) * 2.0 + 1.0;
  vec4 s1 = floor(b1) * 2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));

  vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;

  vec3 p0 = vec3(a0.xy, h.x);
  vec3 p1 = vec3(a0.zw, h.y);
  vec3 p2 = vec3(a1.xy, h.z);
  vec3 p3 = vec3(a1.zw, h.w);

  vec4 norm = taylorInvSqrt(vec4(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
  p0 *= norm.x;
  p1 *= norm.y;
  p2 *= norm.z;
  p3 *= norm.w;

  vec4 m = max(0.6 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), 0.0);
  m = m * m;
  return 42.0 * dot(m * m, vec4(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

float triNoise(vec3 p, float time) {
  float z = 0.4;
  float rz = 0.1;
  vec3 bp = p;
  float shift = time * 0.1 * 0.1;
  for (int i = 0; i < 5; i++) {
    vec3 q = p * 0.01;
    vec3 dg = vec3(tri(q.z + tri(q.y * 20.0)), tri(q.z + tri(q.x)), tri(q.y + tri(q.x)));
    p += dg + shift;
    bp *= 4.0;
    z *= 0.9;
    p *= 1.6;
    rz += tri(p.z + tri(0.6 * p.x + 0.1 * tri(p.y))) / z;
  }
  return ss(0.0, 8.0, rz + sin(rz + sin(z) * 2.8) * 2.2);
}

// Искра-луч в направлении angle: зависит только от угла и времени
float spark(float angle, float time) {
  float idx = (angle / PI_GL) / 2.0;
  float s = sin(PI_GL);
  float c = cos(PI_GL);
  vec2 d = vec2(cos(angle), sin(angle));
  float pa1 = atan(d.x * s + d.y * c, d.x * c - d.y * s);
  float idx1 = (pa1 / PI_GL) / 2.0;
  float idx21 = (pa1 / PI_GL + 1.0) / 2.0 * PI_GL;
  float v = triNoise(vec3(idx, 0.0, 0.0), time);
  float seam = ss(0.9, 1.0, sin(idx21));
  if (seam > 0.0) v += (triNoise(vec3(idx1, 0.0, idx1), time) - v) * seam;
  v = v * 0.2 + pow(v, 10.0);
  return ss(0.0, max(v, 1e-5), 0.3) * v;
}

void main() {
  // Координаты относительно центра обложки: лучи расходятся от её краёв
  vec2 rel = gl_FragCoord.xy - uCenter;
  float ux = rel.x / uUnit;
  float uy = rel.y / uUnit;
  float len = length(vec2(ux, uy));
  float angle = atan(uy, ux);
  float sparkV = spark(angle, uTime);

  float radius0 = uRadius;
  float radius1 = uRadius * (1.0 - R1K);
  float radius2 = uRadius * (1.0 - R2K);
  float width0 = CIRCLE_WIDTH_BASE;
  float width1 = CIRCLE_WIDTH_BASE * (1.0 - W1K);
  float width2 = CIRCLE_WIDTH_BASE * (1.0 - W2K);
  float spark0 = SPARK_STRENGTH_BASE * sparkV;
  float spark1 = (SPARK_STRENGTH_BASE - SPARK_STRENGTH_STEP) * sparkV;
  float spark2 = (SPARK_STRENGTH_BASE - 2.0 * SPARK_STRENGTH_STEP) * sparkV;
  float reactK0 = max(uReact.x, uAudio.x * 0.6) * 50.0;
  float reactK1 = max(uReact.y, uAudio.y * 0.6) * 50.0;
  float reactK2 = max(uReact.z, uAudio.z * 0.6) * 50.0;

  float edges0 = radius0 + width0 * 0.5 + spark0 * (1.0 + reactK0 * spark0);
  float edges1 = radius1 + width1 * 0.5 + spark1 * (1.0 + reactK1 * spark1);
  float edges2 = radius2 + width2 * 0.5 + spark2 * (1.0 + reactK2 * spark2);
  float reach = max(edges0, max(edges1, edges2));
  if (len >= reach + 0.3) discard;

  // Общий шум
  float n0 = snoise3(vec3(ux * 1.2, uy * 1.2, uTime * 0.5));

  vec3 rgb = vec3(0.0);
  float alpha = 0.0;
  for (int i = 0; i < 3; i++) {
    if (i >= int(uCircles)) break;
    float fi = float(i);
    float like = (i == 0) ? uReact.x : ((i == 1) ? uReact.y : uReact.z);
    float audio = (i == 0) ? uAudio.x : ((i == 1) ? uAudio.y : uAudio.z);
    vec3 rot = (i == 0) ? ROT0 : ((i == 1) ? ROT1 : ROT2);
    float offset = CIRCLE_OFFSET_BASE + CIRCLE_OFFSET_STEP * fi;
    float sc = sin(uTime * rot.z);
    float cc = cos(uTime * rot.z);
    vec2 shift = vec2(rot.x * cc - rot.y * sc, rot.x * sc + rot.y * cc);
    float scale = 1.0 - like * 0.5;
    float sinT = sin(uTime + offset);
    float strength = max(like, audio);
    float strengthBase = 0.15 * (1.0 + 1.5 * (-sin(uTime * 2.0 + offset * 0.5) * 0.5)) + 0.3 * strength;
    vec3 c1 = uColors[i];
    vec3 c2 = uColors[i + 3];

    // Край слоя посчитан заранее (edges0/1/2 выше)
    float edge = (i == 0) ? edges0 : ((i == 1) ? edges1 : edges2);
    float outer = edge + 0.3 * n0;
    if (len >= outer) continue;
    vec2 lp = vec2(ux * scale + shift.x, uy * scale + shift.y);
    float llen = length(lp);
    float r0 = snoise3(vec3(lp.x * 1.2 + offset, lp.y * 1.2 + offset, uTime * 0.5 + offset)) * 0.5 + 0.5;
    float v0 = ss(r0 + 0.1 + (sinT + 1.0), r0, llen);
    float a = v0 * ss(outer, 0.5, len);
    if (a <= 0.0) continue;
    float d0 = abs(llen - r0);
    float glow = strengthBase / (1.0 + d0 + d0 * 10.0);
    float flash = 0.6 * like * (1.0 - ss(0.2, outer * 0.8, len));
    float m = lp.y * 2.0;
    for (int ch = 0; ch < 3; ch++) {
      float col = clamp(c1[ch] + (c2[ch] - c1[ch]) * m + glow, 0.0, 1.0) + flash;
      rgb[ch] += (col - rgb[ch]) * a;
    }
    alpha = max(alpha, a);
  }
  if (alpha <= 0.0) discard;
  float a = min(alpha, 1.0) * uOpacity;
  gl_FragColor = vec4(clamp(rgb, 0.0, 1.0), a);
}
`;

  const VERT = `attribute vec2 aPos; void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

  $effect(() => {
    // Палитра из обложки: каналы плавно переезжают на новые цвета
    const c = hexToRgb(color);
    state.colors.update(c ? paletteFromRgb(c) : DEFAULT_PALETTE);
  });
  $effect(() => {
    playingFlag = playing;
    if (playing) state.play();
    else state.idle();
    shownAt = performance.now();
  });
  $effect(() => {
    if (likePulse > 0) {
      state.like();
    }
  });

  onMount(() => {
    const gl = (canvasEl.getContext('webgl', {
      alpha: true,
      premultipliedAlpha: false,
      antialias: false,
    }) ?? canvasEl.getContext('experimental-webgl', {
      alpha: true,
      premultipliedAlpha: false,
      antialias: false,
    })) as WebGLRenderingContext | null;
    if (!gl) return;

    const compile = (type: number, src: string) => {
      const sh = gl.createShader(type)!;
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
        console.error('vibe shader:', gl.getShaderInfoLog(sh));
        return null;
      }
      return sh;
    };
    const vs = compile(gl.VERTEX_SHADER, VERT);
    const fs = compile(gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) return;
    const prog = gl.createProgram()!;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      console.error('vibe link:', gl.getProgramInfoLog(prog));
      return;
    }
    gl.useProgram(prog);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(prog, 'aPos');
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    const u = {
      time: gl.getUniformLocation(prog, 'uTime'),
      opacity: gl.getUniformLocation(prog, 'uOpacity'),
      circles: gl.getUniformLocation(prog, 'uCircles'),
      center: gl.getUniformLocation(prog, 'uCenter'),
      unit: gl.getUniformLocation(prog, 'uUnit'),
      radius: gl.getUniformLocation(prog, 'uRadius'),
      colors: gl.getUniformLocation(prog, 'uColors[0]'),
      audio: gl.getUniformLocation(prog, 'uAudio'),
      react: gl.getUniformLocation(prog, 'uReact'),
    };

    const resize = () => {
      canvasEl.width = window.innerWidth;
      canvasEl.height = window.innerHeight;
      gl.viewport(0, 0, canvasEl.width, canvasEl.height);
    };
    resize();
    window.addEventListener('resize', resize);

    const frameMs = 1000 / 25;
    let lastTick = performance.now();
    let nextTick = lastTick;
    const tick = () => {
      const now = performance.now();
      const dt = Math.min(200, now - lastTick);
      lastTick = now;
      nextTick += frameMs;
      if (nextTick < now) nextTick = now + frameMs;

      // Страница оригинала шлёт уровни звука, только пока трек играет
      state.freq = bands(audioEngine.getFrequencyData(), audioEngine.sampleRateHz);
      state.clock += dt;
      state.fireTimers();
      state.update(dt);

      // На паузе пятно доходит до покоя и замирает: кадры больше не считаются
      if (!playingFlag && now - shownAt >= 1000 && state.resting()) return;

      const fade = Math.min(1, (now - shownAt) / 1000);
      gl.uniform1f(u.time, state.time);
      gl.uniform1f(u.opacity, 0.8 * cubicBezier(0.42, 0.0, 0.58, 1.0, fade));
      gl.uniform1f(u.circles, 3);
      // Центр пятна — за обложкой «Сейчас играет»; fallback — центр экрана
      const art = document.querySelector('.artwork-img')?.getBoundingClientRect();
      let cx: number;
      let cy: number;
      let coverHalf: number;
      if (art && art.width > 0) {
        cx = art.left + art.width / 2;
        cy = canvasEl.height - (art.top + art.height / 2);
        coverHalf = Math.hypot(art.width, art.height) / 2;
      } else {
        cx = canvasEl.width / 2;
        cy = canvasEl.height / 2;
        coverHalf = Math.min(canvasEl.width, canvasEl.height) * 0.13;
      }
      const unit = Math.min(canvasEl.width, canvasEl.height) * 0.5 * SCALE;
      gl.uniform2f(u.center, cx, cy);
      gl.uniform1f(u.unit, unit);
      gl.uniform1f(u.radius, (coverHalf * COVER_FIT) / unit);
      gl.uniform3fv(u.colors, new Float32Array(state.colors.value()));
      gl.uniform3f(u.audio, state.audio[0], state.audio[1], state.audio[2]);
      gl.uniform3f(u.react, state.react[0].cur, state.react[1].cur, state.react[2].cur);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    const timer = setInterval(tick, frameMs);

    return () => {
      clearInterval(timer);
      window.removeEventListener('resize', resize);
    };
  });
</script>

<canvas bind:this={canvasEl} class="vibe-canvas"></canvas>

<style>
  .vibe-canvas {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    width: 100%;
    height: 100%;
  }
</style>

<script lang="ts">
  import { Sliders, RotateCcw, Sparkles, Waves, Volume2, Disc, Plus, Trash2, Download, Upload, Copy, Check } from 'lucide-svelte';
  import SpectrumVisualizer from './SpectrumVisualizer.svelte';
  import RadixSlider from './ui/RadixSlider.svelte';
  import RadixSwitch from './ui/RadixSwitch.svelte';
  import RadixSelect from './ui/RadixSelect.svelte';
  import type {
    AudioEffectsConfig,
    CrossfadeCurve,
    MixConfig,
    MixPreset,
  } from '../types';

  interface Props {
    effects: AudioEffectsConfig;
    mix: MixConfig;
    isPlaying: boolean;
    accentColor: string;
    onChange: (next: AudioEffectsConfig) => Promise<void>;
    onMixChange: (next: MixConfig) => void;
  }

  let {
    effects,
    mix,
    isPlaying,
    accentColor,
    onChange,
    onMixChange,
  }: Props = $props();

  const mixModes: { id: MixPreset; label: string; desc: string }[] = [
    {
      id: 'auto',
      label: 'Авто',
      desc: 'Сам выбирает момент (старт на бите) и эффект: Beatmatch / Rise / Muffle — по темпу и тональности',
    },
    { id: 'fade', label: 'Кроссфейд', desc: 'Ручной режим: ваша кривая и длительность, без авто-эффектов' },
  ];

  const curveOptions: { value: CrossfadeCurve; label: string }[] = [
    { value: 'equal-power', label: 'Плавный (Equal Power)' },
    { value: 's-curve', label: 'Естественный (S-Curve)' },
    { value: 'linear', label: 'Резкий (Linear)' },
  ];

  const bandFreqs = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
  const bandLabels = ['32', '64', '125', '250', '500', '1k', '2k', '4k', '8k', '16k'];

  const builtInPresets: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    'Bass Boost': [7.5, 6.5, 5.0, 2.5, 0.5, 0, 0.5, 1.5, 2.0, 2.0],
    'Bass Destroyer': [12, 12, 10, 7, 3, 0, -2, -4, -4, -5],
    'Treble Boost': [0, 0, 0, 0, 0, 0, 2, 4, 5, 6],
    Electronic: [5.5, 4.5, 2.0, 0, -1.5, 1.5, 3.0, 4.5, 5.5, 6.0],
    'Lo-Fi': [4.0, 3.5, 2.0, 1.0, 0.5, -1.0, -2.5, -4.0, -5.5, -6.5],
    Vocal: [-2.0, -1.5, -0.5, 2.0, 4.5, 5.5, 4.5, 2.5, 0.5, -1.0],
    Rock: [5.0, 3.5, 1.5, -1.0, -1.5, 0.5, 3.0, 4.5, 5.0, 4.5],
    Classical: [0, 0, 0, 0, 0, 0, -2, -3, -3, -4],
    Loudness: [5, 4, 1, 0, -1, 0, -1, 0, 3, 4],
    'V-Shape': [5, 3, 1, -1, -3, -3, -1, 1, 3, 5],
    Gaming: [2.5, 3.0, 0.5, -2.0, -1.5, 1.0, 3.5, 4.5, 3.0, 1.5],
    'Late Night': [3.0, 2.0, 0.5, -1.0, -1.0, 0, 1.0, 1.5, 0.5, -1.0],
  };

  let transferMode = $state<'import' | 'export' | null>(null);
  let transferText = $state('');
  let transferMsg = $state('');
  let copiedApo = $state(false);
  let draggingBand = $state<number | null>(null);
  let curveSvgEl = $state<SVGSVGElement | undefined>(undefined);

  function exportApoText(): string {
    const preamp = (effects.preamp_db ?? 0).toFixed(1);
    const lines = [`Preamp: ${preamp} dB`];
    effects.bands.forEach((db, i) => {
      lines.push(`Filter ${i + 1}: ON PK Fc ${bandFreqs[i]} Hz Gain ${db.toFixed(1)} dB Q 1.41`);
    });
    return lines.join('\n');
  }

  function openExport() {
    if (transferMode === 'export') {
      transferMode = null;
      return;
    }
    transferText = exportApoText();
    transferMsg = '';
    transferMode = 'export';
  }

  function openImport() {
    if (transferMode === 'import') {
      transferMode = null;
      return;
    }
    transferText = '';
    transferMsg = '';
    transferMode = 'import';
  }

  async function copyExport() {
    try {
      await navigator.clipboard.writeText(transferText);
      copiedApo = true;
      setTimeout(() => (copiedApo = false), 1500);
    } catch {
      // ignore
    }
  }

  function applyImportedApo() {
    const raw = transferText.trim();
    if (!raw) return;
    const nextBands = [...effects.bands];
    let nextPreamp = effects.preamp_db ?? 0;
    let matched = 0;

    const preampMatch = raw.match(/Preamp:\s*(-?\d+(?:\.\d+)?)\s*dB/i);
    if (preampMatch) {
      nextPreamp = Math.max(-24, Math.min(6, Math.round(Number(preampMatch[1]) * 2) / 2));
    }

    // Equalizer APO Filter lines: Filter N: ON PK Fc 1000 Hz Gain 3.5 dB ...
    const filterRe = /Fc\s+(\d+(?:\.\d+)?)\s*Hz.*?Gain\s+(-?\d+(?:\.\d+)?)\s*dB/gi;
    let m: RegExpExecArray | null;
    while ((m = filterRe.exec(raw)) !== null) {
      const freq = Number(m[1]);
      const gain = Math.max(-12, Math.min(12, Math.round(Number(m[2]) * 2) / 2));
      let bestIdx = 0;
      let bestDiff = Infinity;
      bandFreqs.forEach((bf, idx) => {
        const d = Math.abs(Math.log2(freq / bf));
        if (d < bestDiff) {
          bestDiff = d;
          bestIdx = idx;
        }
      });
      nextBands[bestIdx] = gain;
      matched++;
    }

    // GraphicEQ fallback: GraphicEQ: 32 -2; 64 1.5; ...
    if (matched === 0) {
      const pairRe = /(\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g;
      while ((m = pairRe.exec(raw)) !== null) {
        const freq = Number(m[1]);
        const gain = Math.max(-12, Math.min(12, Math.round(Number(m[2]) * 2) / 2));
        let bestIdx = 0;
        let bestDiff = Infinity;
        bandFreqs.forEach((bf, idx) => {
          const d = Math.abs(Math.log2(freq / bf));
          if (d < bestDiff) {
            bestDiff = d;
            bestIdx = idx;
          }
        });
        nextBands[bestIdx] = gain;
        matched++;
      }
    }

    if (matched === 0 && !preampMatch) {
      transferMsg = 'Не удалось распознать формат Equalizer APO или GraphicEQ.';
      return;
    }

    update({ bands: nextBands, preamp_db: nextPreamp, preset: 'Custom', eq_enabled: true });
    transferMode = null;
    transferMsg = '';
  }

  function updateBandFromSvgPointer(e: PointerEvent, bandIdx: number) {
    if (!curveSvgEl || !effects.eq_enabled) return;
    const rect = curveSvgEl.getBoundingClientRect();
    const relY = Math.max(12, Math.min(rect.height - 12, e.clientY - rect.top));
    const norm = (relY - 12) / Math.max(1, rect.height - 24);
    const db = Math.max(-12, Math.min(12, Math.round((12 - norm * 24) * 2) / 2));
    setBand(bandIdx, db);
  }

  let customPresets = $state<Record<string, number[]>>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_custom_eq_presets') : null;
        return raw ? JSON.parse(raw) : {};
      } catch {
        return {};
      }
    })()
  );
  let newPresetName = $state('');

  const allPresets = $derived({ ...builtInPresets, ...customPresets });

  function saveCustomPresets(next: Record<string, number[]>) {
    customPresets = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_custom_eq_presets', JSON.stringify(next));
    }
  }

  function handleSavePreset() {
    const name = newPresetName.trim();
    if (!name || builtInPresets[name]) return;
    saveCustomPresets({ ...customPresets, [name]: [...effects.bands] });
    newPresetName = '';
    update({ preset: name, eq_enabled: true });
  }

  function handleDeletePreset(name: string) {
    const copy = { ...customPresets };
    delete copy[name];
    saveCustomPresets(copy);
    if (effects.preset === name) {
      update({ preset: 'Custom' });
    }
  }

  function update(patch: Partial<AudioEffectsConfig>) {
    onChange({ ...effects, ...patch });
  }

  function setBand(idx: number, val: number) {
    const nextBands = [...effects.bands];
    nextBands[idx] = val;
    update({ bands: nextBands, preset: 'Custom' });
  }

  function applyPreset(name: string) {
    const curve = allPresets[name];
    if (!curve) return;
    update({ preset: name, bands: [...curve], eq_enabled: true });
  }

  const svgPoints = $derived.by(() => {
    const w = 600;
    const h = 140;
    const step = w / 9;
    return effects.bands.map((db, i) => {
      const x = i * step;
      const norm = (12 - db) / 24;
      const y = 12 + norm * (h - 24);
      return [x, y] as const;
    });
  });

  const svgPath = $derived.by(() => {
    return svgPoints
      .map(([x, y], i) => {
        if (i === 0) return `M ${x} ${y}`;
        const [px, py] = svgPoints[i - 1];
        const cx = (px + x) / 2;
        return `C ${cx} ${py}, ${cx} ${y}, ${x} ${y}`;
      })
      .join(' ');
  });

  const svgAreaPath = $derived(`${svgPath} L 600 140 L 0 140 Z`);
</script>

<section class="eq-page">
  <div class="top-row">
    <div class="title-block">
      <div class="eq-icon">
        <Sliders size={20} />
      </div>
      <div>
        <h2>Эквалайзер и DSP-обработка</h2>
        <p>10-полосный параметрический фильтр, плавный кроссфейд и расширение стереосцены</p>
      </div>
    </div>

    <div class="top-actions">
      <button type="button" class="reset-btn" class:active-apo={transferMode === 'import'} onclick={openImport}>
        <Upload size={13} />
        <span>Импорт APO</span>
      </button>
      <button type="button" class="reset-btn" class:active-apo={transferMode === 'export'} onclick={openExport}>
        <Download size={13} />
        <span>Экспорт APO</span>
      </button>
      <button
        class="toggle-pill"
        class:active={effects.eq_enabled}
        onclick={() => update({ eq_enabled: !effects.eq_enabled })}
      >
        <span>{effects.eq_enabled ? 'EQ Включён' : 'EQ Выключен'}</span>
      </button>

      <button class="reset-btn" onclick={() => update({ preset: 'Flat', bands: [...builtInPresets.Flat], preamp_db: 0, eq_enabled: true })}>
        <RotateCcw size={14} />
        <span>Сброс</span>
      </button>
    </div>
  </div>

  <!-- Живой спектр -->
  <div class="spectrum-card">
    <div class="spectrum-head">
      <span>Живой спектр (FFT)</span>
      <span class="preset-badge">{effects.preset}</span>
    </div>
    <SpectrumVisualizer
      {isPlaying}
      {accentColor}
      bands={effects.bands}
      eqEnabled={effects.eq_enabled}
      height={64}
    />
  </div>

  <!-- Пресеты -->
  <div class="presets-bar">
    {#each Object.keys(allPresets) as name (name)}
      <div class="preset-item">
        <button
          class="preset-chip"
          class:active={effects.preset === name}
          onclick={() => applyPreset(name)}
        >
          {name}
        </button>
        {#if customPresets[name]}
          <button
            type="button"
            class="preset-del"
            onclick={() => handleDeletePreset(name)}
            title="Удалить пресет"
            aria-label="Удалить пресет"
          >
            <Trash2 size={11} />
          </button>
        {/if}
      </div>
    {/each}

    <form
      class="save-preset-form"
      onsubmit={(e) => {
        e.preventDefault();
        handleSavePreset();
      }}
    >
      <input
        type="text"
        bind:value={newPresetName}
        placeholder="Новый пресет..."
        maxlength="24"
      />
      <button type="submit" class="save-preset-btn" title="Сохранить текущие настройки">
        <Plus size={13} />
        <span>Сохранить</span>
      </button>
    </form>
  </div>

  <!-- Импорт / Экспорт Equalizer APO -->
  {#if transferMode}
    <div class="apo-card">
      <div class="apo-head">
        <span>{transferMode === 'export' ? 'Экспорт в формат Equalizer APO' : 'Импорт Equalizer APO / GraphicEQ'}</span>
        {#if transferMode === 'export'}
          <button type="button" class="reset-btn" onclick={copyExport}>
            {#if copiedApo}<Check size={13} /><span>Скопировано</span>{:else}<Copy size={13} /><span>Копировать</span>{/if}
          </button>
        {:else}
          <button type="button" class="save-preset-btn" onclick={applyImportedApo}>Применить</button>
        {/if}
      </div>
      <textarea
        class="apo-area"
        rows="4"
        readonly={transferMode === 'export'}
        bind:value={transferText}
        placeholder="Вставьте строки Equalizer APO (Preamp / Filter ...) или GraphicEQ..."
      ></textarea>
      {#if transferMsg}<div class="apo-err">{transferMsg}</div>{/if}
    </div>
  {/if}

  <!-- 10-полосный эквалайзер + Preamp + интерактивная кривая -->
  <div class="eq-board-wrap" class:disabled={!effects.eq_enabled}>
    <div class="preamp-col">
      <span class="db-val">{(effects.preamp_db ?? 0) > 0 ? `+${(effects.preamp_db ?? 0).toFixed(1)}` : (effects.preamp_db ?? 0).toFixed(1)}</span>
      <div class="v-slider-wrap">
        <RadixSlider
          value={effects.preamp_db ?? 0}
          min={-24}
          max={6}
          step={0.5}
          orientation="vertical"
          disabled={!effects.eq_enabled}
          ariaLabel="Preamp"
          onValueChange={(v) => update({ preamp_db: v })}
        />
      </div>
      <span class="freq-lbl">PRE</span>
    </div>

    <div class="eq-board">
      <svg
        bind:this={curveSvgEl}
        class="curve-svg"
        viewBox="0 0 600 140"
        preserveAspectRatio="none"
        role="presentation"
        onpointermove={(e) => {
          if (draggingBand !== null) updateBandFromSvgPointer(e, draggingBand);
        }}
        onpointerup={() => (draggingBand = null)}
        onpointerleave={() => (draggingBand = null)}
      >
        <defs>
          <linearGradient id="eqAreaGrad" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color={accentColor} stop-opacity="0.24" />
            <stop offset="100%" stop-color={accentColor} stop-opacity="0.0" />
          </linearGradient>
        </defs>
        <line x1="0" y1="70" x2="600" y2="70" stroke="rgba(255,255,255,0.08)" stroke-dasharray="4 4" />
        <path d={svgAreaPath} fill="url(#eqAreaGrad)" />
        <path d={svgPath} fill="none" stroke={accentColor} stroke-width="2.5" opacity="0.9" />
        {#each svgPoints as [cx, cy], idx (idx)}
          <circle
            cx={cx}
            cy={cy}
            r={draggingBand === idx ? 6 : 4.5}
            fill="#fff"
            stroke={accentColor}
            stroke-width="2"
            class="curve-dot"
            role="slider"
            tabindex="0"
            aria-label={`Кривая ${bandLabels[idx]} Гц`}
            aria-valuemin={-12}
            aria-valuemax={12}
            aria-valuenow={effects.bands[idx]}
            onpointerdown={(e) => {
              e.stopPropagation();
              draggingBand = idx;
              (e.currentTarget as Element).setPointerCapture(e.pointerId);
              updateBandFromSvgPointer(e, idx);
            }}
          />
        {/each}
      </svg>

      <div class="sliders-grid">
        {#each effects.bands as val, idx (idx)}
          <div class="band-col">
            <span class="db-val">{val > 0 ? `+${val.toFixed(1)}` : val.toFixed(1)}</span>
            <div class="v-slider-wrap">
              <RadixSlider
                value={val}
                min={-12}
                max={12}
                step={0.5}
                orientation="vertical"
                disabled={!effects.eq_enabled}
                ariaLabel={`Полоса ${bandLabels[idx]}`}
                onValueChange={(v) => setBand(idx, v)}
              />
            </div>
            <span class="freq-lbl">{bandLabels[idx]}</span>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <!-- Инструмент Mix (Auto-Mix / Ручной) -->
  <div class="mix-card">
    <div class="mix-head">
      <div class="mix-title-block">
        <div class="feat-title">
          <Disc size={16} />
          <span>Mix & Auto-Mix переходы</span>
        </div>
        <span class="mix-subtitle">Авто-DJ сам выбирает момент и эффект перехода; Кроссфейд — ручной режим с вашей кривой</span>
      </div>
      <div class="mix-switch-wrap">
        <span class="mix-state-lbl">{mix.enabled ? 'Вкл' : 'Выкл'}</span>
        <RadixSwitch
          checked={mix.enabled}
          ariaLabel="Включить Mix & Auto-Mix"
          onCheckedChange={(v) => onMixChange({ ...mix, enabled: v })}
        />
      </div>
    </div>

    {#if mix.enabled}
      <div class="mix-presets-row">
        {#each mixModes as mp (mp.id)}
          <button
            type="button"
            class="mix-preset-btn"
            class:active={mix.preset === mp.id}
            onclick={() => onMixChange({ ...mix, preset: mp.id })}
          >
            <span class="mp-name">{mp.label}</span>
            <span class="mp-desc">{mp.desc}</span>
          </button>
        {/each}
      </div>

      <div class="mix-setting-row">
        <div class="mix-setting-info">
          <span class="rg-lbl">Кривая кроссфейда</span>
          <span class="mix-setting-hint">{mix.preset === 'auto' ? 'Подбирается алгоритмом вместе с эффектом' : 'Характер затухания и нарастания громкости'}</span>
        </div>
        <div style="min-width: 220px;" class:mix-disabled={mix.preset === 'auto'}>
          <RadixSelect
            value={mix.curve}
            options={curveOptions}
            disabled={mix.preset === 'auto'}
            onValueChange={(v) => onMixChange({ ...mix, curve: v as CrossfadeCurve })}
          />
        </div>
      </div>

      <div class="mix-slider" class:mix-disabled={mix.preset === 'auto'}>
        <span>{mix.preset === 'auto' ? 'Базовая длительность (алгоритм подстроит):' : 'Длительность перехода:'} <b>{mix.durationSec} сек</b></span>
        <RadixSlider
          value={mix.durationSec}
          min={1}
          max={12}
          step={1}
          disabled={mix.preset === 'auto'}
          ariaLabel="Длительность перехода"
          onValueChange={(v) => onMixChange({ ...mix, durationSec: v })}
        />
      </div>
    {/if}

      </div>

  <!-- Дополнительные фишки -->
  <div class="features-grid">
    <div class="feat-card">
      <div class="feat-top">
        <div class="feat-title">
          <Waves size={16} />
          <span>Bass Boost</span>
        </div>
        <span class="feat-val">{effects.bass_boost}%</span>
      </div>
      <RadixSlider
        value={effects.bass_boost}
        min={0}
        max={100}
        step={5}
        ariaLabel="Bass Boost"
        onValueChange={(v) => update({ bass_boost: v })}
      />
      <p>Усиление суббаса (до 80 Гц) без искажения вокала.</p>
    </div>

    <button
      class="feat-card toggleable"
      class:on={effects.normalize_volume}
      onclick={() => update({ normalize_volume: !effects.normalize_volume })}
    >
      <div class="feat-top">
        <div class="feat-title">
          <Volume2 size={16} />
          <span>Нормализация громкости</span>
        </div>
        <span class="switch-dot">{effects.normalize_volume ? 'ВКЛ' : 'ВЫКЛ'}</span>
      </div>
      <p>Выравнивает громкость между Яндекс Музыкой и SoundCloud.</p>
    </button>

    <button
      class="feat-card toggleable"
      class:on={effects.spatial_audio}
      onclick={() => update({ spatial_audio: !effects.spatial_audio })}
    >
      <div class="feat-top">
        <div class="feat-title">
          <Sparkles size={16} />
          <span>Spatial 3D-сцена</span>
        </div>
        <span class="switch-dot">{effects.spatial_audio ? 'ВКЛ' : 'ВЫКЛ'}</span>
      </div>
      <p>Лёгкое расширение стереобазы для наушников.</p>
    </button>
  </div>
</section>

<style>
  .eq-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    overflow-x: hidden;
  }

  .top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .title-block {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .eq-icon {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--text-primary);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .title-block h2 {
    font-size: 17px;
    font-weight: 700;
  }

  .title-block p {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .top-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toggle-pill,
  .reset-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 13px;
    border-radius: 10px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-secondary);
  }

  .toggle-pill.active {
    background: var(--accent);
    color: #09090d;
    border-color: transparent;
  }

  .reset-btn:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .spectrum-card {
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .spectrum-head {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .preset-badge {
    color: var(--accent);
  }

  .presets-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .preset-item {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }

  .preset-del {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: none;
    background: transparent;
    color: var(--text-muted);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .preset-del:hover {
    color: #fb7185;
    background: rgba(244, 63, 94, 0.14);
  }

  .save-preset-form {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .save-preset-form input {
    width: 130px;
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 11px;
  }

  .save-preset-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .save-preset-btn:hover {
    border-color: var(--accent);
  }

  .preset-chip {
    padding: 6px 12px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 500;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .preset-chip:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.09);
  }

  .preset-chip.active {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .eq-board {
    position: relative;
    padding: 18px 16px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    transition: opacity 0.2s ease;
  }

  .curve-svg {
    position: relative;
    width: 100%;
    height: 112px;
    margin-bottom: 10px;
    cursor: crosshair;
  }

  .curve-dot {
    cursor: grab;
    transition: r 100ms ease;
  }

  .curve-dot:active {
    cursor: grabbing;
  }

  .eq-board-wrap {
    display: grid;
    grid-template-columns: 56px 1fr;
    gap: 12px;
    align-items: stretch;
  }

  .eq-board-wrap.disabled {
    opacity: 0.45;
    pointer-events: none;
  }

  .preamp-col {
    padding: 18px 8px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .apo-card {
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .apo-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .apo-area {
    width: 100%;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.7);
    color: var(--text-primary);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
    resize: vertical;
    outline: none;
  }

  .apo-err {
    font-size: 11.5px;
    color: #f87171;
  }

  .reset-btn.active-apo {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .sliders-grid {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 6px;
  }

  .band-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .db-val {
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .v-slider-wrap {
    position: relative;
    width: 100%;
    height: 136px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .mix-card {
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .mix-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }

  .mix-title-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .mix-subtitle {
    font-size: 11px;
    color: var(--text-muted);
  }

  .mix-switch-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mix-state-lbl {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }

    .mix-setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .mix-setting-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .mix-setting-hint {
    font-size: 11px;
    color: var(--text-muted);
  }

        .mix-presets-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(145px, 1fr));
    gap: 8px;
  }

  .mix-preset-btn {
    padding: 8px 10px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.02);
    text-align: left;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .mix-preset-btn.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, var(--bg-elevated));
  }

  .mp-name {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .mp-desc {
    font-size: 10px;
    color: var(--text-secondary);
  }

  .mix-disabled {
    opacity: 0.45;
  }

  .mix-slider {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .freq-lbl {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .features-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  .feat-card {
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-align: left;
    color: inherit;
  }

  .feat-card.toggleable {
    cursor: pointer;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .feat-card.toggleable.on {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-surface));
  }

  .feat-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .feat-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 600;
  }

  .feat-val {
    font-size: 12px;
    font-weight: 700;
    color: var(--accent);
  }

  .switch-dot {
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-secondary);
  }

  .feat-card.toggleable.on .switch-dot {
    background: var(--accent);
    color: #09090d;
  }

  .feat-card p {
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-muted);
  }
</style>

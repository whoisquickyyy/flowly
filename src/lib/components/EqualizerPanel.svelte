<script lang="ts">
  import { Sliders, RotateCcw, Sparkles, Waves, Volume2, Disc, Plus, Trash2 } from 'lucide-svelte';
  import SpectrumVisualizer from './SpectrumVisualizer.svelte';
  import type {
    AudioEffectsConfig,
    ManualTransitionStyle,
    MixConfig,
    MixPreset,
    ShuffleMode,
  } from '../types';

  interface Props {
    effects: AudioEffectsConfig;
    mix: MixConfig;
    shuffleMode?: ShuffleMode;
    isPlaying: boolean;
    accentColor: string;
    onChange: (next: AudioEffectsConfig) => Promise<void>;
    onMixChange: (next: MixConfig) => void;
    onShuffleModeChange?: (mode: ShuffleMode) => void;
  }

  let {
    effects,
    mix,
    shuffleMode = 'classic',
    isPlaying,
    accentColor,
    onChange,
    onMixChange,
    onShuffleModeChange = () => {},
  }: Props = $props();

  const manualSkipOptions: { id: ManualTransitionStyle; label: string }[] = [
    { id: 'filter-cut', label: 'Filter Cut' },
    { id: 'echo-tail', label: 'Echo Tail' },
    { id: 'vinyl-brake', label: 'Винил' },
    { id: 'tape-whoosh', label: 'Air Whoosh' },
    { id: 'clean-whip', label: 'Clean Whip' },
    { id: 'off', label: 'Мгновенно' },
  ];

  const mixPresets: { id: MixPreset; label: string; desc: string }[] = [
    { id: 'dj-vocal', label: 'Beat + Vocal', desc: 'Старый бит + вокал нового трека' },
    { id: 'bass-swap', label: 'Bass Swap', desc: 'DJ-смена баса в середине' },
    { id: 'rise', label: 'Rise', desc: 'Нарастающий фильтр на входе' },
    { id: 'echo-out', label: 'Echo Out', desc: 'Мягкое затухание верхов' },
    { id: 'fade', label: 'Fade', desc: 'Плавный громкостной переход' },
  ];

  const bandLabels = ['32', '64', '125', '250', '500', '1k', '2k', '4k', '8k', '16k'];

  const builtInPresets: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    'Bass Boost': [7.5, 6.5, 5.0, 2.5, 0.5, 0, 0.5, 1.5, 2.0, 2.0],
    Electronic: [5.5, 4.5, 2.0, 0, -1.5, 1.5, 3.0, 4.5, 5.5, 6.0],
    'Lo-Fi': [4.0, 3.5, 2.0, 1.0, 0.5, -1.0, -2.5, -4.0, -5.5, -6.5],
    Vocal: [-2.0, -1.5, -0.5, 2.0, 4.5, 5.5, 4.5, 2.5, 0.5, -1.0],
    Rock: [5.0, 3.5, 1.5, -1.0, -1.5, 0.5, 3.0, 4.5, 5.0, 4.5],
    Gaming: [2.5, 3.0, 0.5, -2.0, -1.5, 1.0, 3.5, 4.5, 3.0, 1.5],
    'Late Night': [3.0, 2.0, 0.5, -1.0, -1.0, 0, 1.0, 1.5, 0.5, -1.0],
  };

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

  const svgPath = $derived.by(() => {
    const w = 600;
    const h = 140;
    const step = w / 9;
    const points = effects.bands.map((db, i) => {
      const x = i * step;
      const norm = (12 - db) / 24;
      const y = 12 + norm * (h - 24);
      return [x, y] as const;
    });
    return points
      .map(([x, y], i) => {
        if (i === 0) return `M ${x} ${y}`;
        const [px, py] = points[i - 1];
        const cx = (px + x) / 2;
        return `C ${cx} ${py}, ${cx} ${y}, ${x} ${y}`;
      })
      .join(' ');
  });
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
      <button
        class="toggle-pill"
        class:active={effects.eq_enabled}
        onclick={() => update({ eq_enabled: !effects.eq_enabled })}
      >
        <span>{effects.eq_enabled ? 'EQ Включён' : 'EQ Выключен'}</span>
      </button>

      <button class="reset-btn" onclick={() => applyPreset('Flat')}>
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

  <!-- 10-полосный эквалайзер -->
  <div class="eq-board" class:disabled={!effects.eq_enabled}>
    <svg class="curve-svg" viewBox="0 0 600 140" preserveAspectRatio="none">
      <line x1="0" y1="70" x2="600" y2="70" stroke="rgba(255,255,255,0.08)" stroke-dasharray="4 4" />
      <path d={svgPath} fill="none" stroke={accentColor} stroke-width="2.5" opacity="0.85" />
    </svg>

    <div class="sliders-grid">
      {#each effects.bands as val, idx (idx)}
        <div class="band-col">
          <span class="db-val">{val > 0 ? `+${val.toFixed(1)}` : val.toFixed(1)}</span>
          <div class="v-slider-wrap">
            <input
              type="range"
              min="-12"
              max="12"
              step="0.5"
              value={val}
              disabled={!effects.eq_enabled}
              oninput={(e) => setBand(idx, Number((e.target as HTMLInputElement).value))}
            />
          </div>
          <span class="freq-lbl">{bandLabels[idx]}</span>
        </div>
      {/each}
    </div>
  </div>

  <!-- Инструмент Mix (Auto-Mix / Ручной) -->
  <div class="mix-card">
    <div class="mix-head">
      <div class="feat-title">
        <Disc size={16} />
        <span>Mix & Auto-Mix переходы</span>
      </div>
      <div class="mix-controls-top">
        <div class="mix-mode-pills">
          <button
            type="button"
            class:active={mix.mode === 'auto'}
            onclick={() => onMixChange({ ...mix, enabled: true, mode: 'auto' })}
          >
            Auto
          </button>
          <button
            type="button"
            class:active={mix.mode === 'manual'}
            onclick={() => onMixChange({ ...mix, enabled: true, mode: 'manual' })}
          >
            Ручной
          </button>
        </div>
        <button
          type="button"
          class="switch-dot-btn"
          class:on={mix.enabled}
          onclick={() => onMixChange({ ...mix, enabled: !mix.enabled })}
        >
          {mix.enabled ? 'ВКЛ' : 'ВЫКЛ'}
        </button>
      </div>
    </div>

    <div class="mix-presets-row">
      {#each mixPresets as mp (mp.id)}
        <button
          type="button"
          class="mix-preset-btn"
          class:active={mix.preset === mp.id}
          onclick={() => onMixChange({ ...mix, enabled: true, preset: mp.id })}
        >
          <span class="mp-name">{mp.label}</span>
          <span class="mp-desc">{mp.desc}</span>
        </button>
      {/each}
    </div>

    <div class="mix-bottom-row">
      {#if mix.mode === 'manual'}
        <div class="mix-slider">
          <span>Длительность: <b>{mix.durationSec} сек</b></span>
          <input
            type="range"
            min="2"
            max="12"
            step="1"
            value={mix.durationSec}
            oninput={(e) =>
              onMixChange({ ...mix, durationSec: Number((e.target as HTMLInputElement).value) })}
          />
        </div>
      {:else}
        <span class="mix-auto-note">
          Auto-DJ сам выбирает эффект перехода (включая Beat + Vocal, Bass Swap, Rise) и точку входа.
        </span>
      {/if}

      <button
        type="button"
        class="preset-chip"
        class:active={mix.eqSweep}
        onclick={() => onMixChange({ ...mix, eqSweep: !mix.eqSweep })}
      >
        EQ-фильтр перехода: {mix.eqSweep ? 'Вкл' : 'Выкл'}
      </button>
    </div>

    <div class="manual-skip-row">
      <span class="rg-lbl">Переход при ручном скипе:</span>
      <div class="mix-mode-pills">
        {#each manualSkipOptions as opt (opt.id)}
          <button
            type="button"
            class:active={(mix.manualTransition ?? 'filter-cut') === opt.id}
            onclick={() => onMixChange({ ...mix, manualTransition: opt.id })}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="manual-skip-row">
      <span class="rg-lbl">Режим шафла:</span>
      <div class="mix-mode-pills">
        <button
          type="button"
          class:active={shuffleMode === 'classic'}
          onclick={() => onShuffleModeChange('classic')}
        >
          Обычный шафл
        </button>
        <button
          type="button"
          class:active={shuffleMode === 'bpm-flow'}
          onclick={() => onShuffleModeChange('bpm-flow')}
        >
          BPM Flow
        </button>
      </div>
    </div>
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
      <input
        type="range"
        min="0"
        max="100"
        step="5"
        value={effects.bass_boost}
        oninput={(e) => update({ bass_boost: Number((e.target as HTMLInputElement).value) })}
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
      <p>Выравнивает громкость между Яндекс Музыкой, Spotify и SoundCloud.</p>
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

  .eq-board.disabled {
    opacity: 0.45;
  }

  .curve-svg {
    position: absolute;
    left: 24px;
    right: 24px;
    top: 38px;
    width: calc(100% - 48px);
    height: 140px;
    pointer-events: none;
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
    overflow: hidden;
  }

  .v-slider-wrap input[type='range'] {
    position: absolute;
    width: 124px;
    height: 24px;
    transform: rotate(-90deg);
    cursor: pointer;
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

  .mix-controls-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .mix-mode-pills {
    display: flex;
    gap: 4px;
    padding: 3px;
    border-radius: 999px;
    background: var(--bg-graphite);
    border: 1px solid var(--border-subtle);
  }

  .mix-mode-pills button {
    padding: 4px 11px;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .mix-mode-pills button.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .switch-dot-btn {
    font-size: 10px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    cursor: pointer;
  }

  .switch-dot-btn.on {
    background: var(--accent);
    color: #09090d;
    border-color: transparent;
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

  .mix-bottom-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .manual-skip-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .mix-slider {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .mix-auto-note {
    font-size: 11px;
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

<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { listen } from '@tauri-apps/api/event';
  import MarqueeText from './lib/components/MarqueeText.svelte';
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Shuffle,
    Repeat,
    Repeat1,
    Volume2,
    VolumeX,
    Sliders,
    ListMusic,
    Minus,
    Square,
    X,
    Disc3,
    Library,
    Search,
    Settings,
    Palette,
    LayoutGrid,
    BarChart3,
    Radio,
    User,
    Heart,
    Info,
    ExternalLink,
    Shield,
    Download,
    RefreshCw,
    Sparkles,
  } from 'lucide-svelte';

  import DetailsView from './lib/components/DetailsView.svelte';
  import LibraryView from './lib/components/LibraryView.svelte';
  import SearchView from './lib/components/SearchView.svelte';
  import PlaylistsView from './lib/components/PlaylistsView.svelte';
  import SourceBadge from './lib/components/SourceBadge.svelte';
  import FlowlyLogo from './lib/components/FlowlyLogo.svelte';
  import ArtistView from './lib/components/ArtistView.svelte';
  import AccountModal from './lib/components/AccountModal.svelte';
  import ReactiveBackground from './lib/components/ReactiveBackground.svelte';
  import WeatherAtmosphereLayer from './lib/components/WeatherAtmosphereLayer.svelte';
  import RadixSlider from './lib/components/ui/RadixSlider.svelte';
  import RadixDialog from './lib/components/ui/RadixDialog.svelte';
  import './lib/styles/radix-ui.css';
  import * as sb from './lib/supabase';
  import type { FlowlyProfile, SupabaseSession } from './lib/supabase';
  import {
    buildBlendedQueue,
    generateRoomCode,
    trackMatchKey,
    type BlendedTrack,
    type JamChatMessage,
    type JamMember,
    type JamRoomState,
  } from './lib/jamRoom';
  import { audioEngine } from './lib/audioEngine';
  import { getTrackAudioFeatures, getHarmonicCrossfadeSec } from './lib/camelot';
  import { extractCoverPalette, type CoverPalette } from './lib/colorExtract';
  import * as api from './lib/api';
  import type {
    AudioEffectsConfig,
    BackgroundMode,
    LayoutStyle,
    MixConfig,
    MixPreset,
    PlayerState,
    Playlist,
    RadioHostConfig,
    SourceStatus,
    Track,
    WaveConfig,
    WeatherAtmosphere,
    ActivityCategory,
    ActivityConfig,
  } from './lib/types';

  type TabId = 'details' | 'library' | 'search' | 'playlists' | 'recap' | 'room' | 'equalizer' | 'settings';

  const ALL_SOURCES: import('./lib/types').SourceKind[] = ['yandex', 'soundcloud'];

  let enabledSources = $state<import('./lib/types').SourceKind[]>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_enabled_sources') : null;
        if (raw) {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed) && parsed.length > 0) return parsed;
        }
      } catch {
        // ignore
      }
      return [...ALL_SOURCES];
    })()
  );

  function updateEnabledSources(next: import('./lib/types').SourceKind[]) {
    const safe = next.length > 0 ? next : [...ALL_SOURCES];
    enabledSources = safe;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_enabled_sources', JSON.stringify(safe));
    }
    if (safe.length === 1) {
      updateWaveConfig({ ...waveConfig, sourceFilter: safe[0] });
    } else if (waveConfig.sourceFilter && !safe.includes(waveConfig.sourceFilter)) {
      updateWaveConfig({ ...waveConfig, sourceFilter: null });
    }
  }

  let activeTab = $state<TabId>('details');
  let quickSearch = $state('');
  let sources = $state<SourceStatus[]>([]);
  let playlists = $state<Playlist[]>([]);
  let effects = $state<AudioEffectsConfig>({
    eq_enabled: true,
    preset: 'Flat',
    bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    crossfade_sec: 4,
    normalize_volume: true,
    bass_boost: 0,
    spatial_audio: false,
  });
  let player = $state<PlayerState>({
    current_track: null,
    is_playing: false,
    position_sec: 0,
    volume: 80,
    shuffle: false,
    repeat: 'all',
    queue: [],
  });

  let audioA: HTMLAudioElement | undefined = $state();
  let audioB: HTMLAudioElement | undefined = $state();
  let activeDeck = $state<'A' | 'B'>('A');
  let lastLoadedUrl = $state<string | null>(null);
  let crossfadeTriggeredForId = $state<string | null>(null);
  let isNaturalEndTransition = false;
  let switchToken = 0;
  let preloadedForTrackId: string | null = null;
  let audioProxyPort = $state(0);

  function getActiveAudio(): HTMLAudioElement | undefined {
    return activeDeck === 'A' ? audioA : audioB;
  }
  let showBgMenu = $state(false);
  let showLayoutMenu = $state(false);
  let likedTracks = $state<Track[]>([]);
  let recentTracks = $state<Track[]>([]);
  let localTracks = $state<Track[]>([]);
  let settingsInitialTab = $state<'services' | 'appearance' | 'sound' | 'network' | 'integrations' | 'account' | 'general' | 'about'>('services');
  let availableUpdateVersion = $state<string | null>(null);
  let availableUpdateNotes = $state<string>('');
  let availableUpdateDate = $state<string | null>(null);
  let showUpdateModal = $state(false);
  let showWhatsNewModal = $state(false);
  let whatsNewVersion = $state('0.1.2');
  let whatsNewNotes = $state('');
  let whatsNewDate = $state<string | null>(null);
  let installingOta = $state(false);
  let updateDownloadPct = $state<number | null>(null);
  let updateInstallError = $state('');

  async function installTopbarUpdate() {
    if (installingOta) return;
    installingOta = true;
    updateInstallError = '';
    updateDownloadPct = null;
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const { relaunch } = await import('@tauri-apps/plugin-process');
      const upd = await check();
      if (upd) {
        try {
          localStorage.setItem(
            'flowly_pending_changelog',
            JSON.stringify({
              version: upd.version,
              notes: upd.body?.trim() ?? '',
              date: upd.date ?? null,
            })
          );
        } catch {
          // ignore
        }
        let totalBytes = 0;
        let downloadedBytes = 0;
        await upd.downloadAndInstall((event) => {
          if (event.event === 'Started') {
            totalBytes = event.data.contentLength ?? 0;
            downloadedBytes = 0;
            updateDownloadPct = totalBytes > 0 ? 0 : null;
          } else if (event.event === 'Progress') {
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              updateDownloadPct = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100));
            }
          } else if (event.event === 'Finished') {
            updateDownloadPct = 100;
          }
        });
        await relaunch();
      } else {
        installingOta = false;
        showUpdateModal = false;
      }
    } catch (e) {
      updateInstallError = String(e);
      installingOta = false;
    }
  }
  let likedIds = $state<Set<string>>(new Set());
  let activeArtist = $state<string | null>(null);
  let sbSession = $state<SupabaseSession | null>(null);
  let sbProfile = $state<FlowlyProfile | null>(sb.getCachedProfile());
  let showAccountModal = $state(false);
  let showOnboarding = $state(
    typeof localStorage !== 'undefined' && localStorage.getItem('flowly_onboarding_done') !== '1'
  );
  let mixConfig = $state<MixConfig>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_mix_config') : null;
        if (raw) return JSON.parse(raw);
      } catch {
        // ignore
      }
      return { enabled: true, mode: 'auto', preset: 'fade', durationSec: 6, eqSweep: true };
    })()
  );

  let lastTrackForMix: Track | null = null;
  let deckCleanupTimers: Record<'A' | 'B', ReturnType<typeof setTimeout> | null> = { A: null, B: null };
  let deckFadeInCleanups: Record<'A' | 'B', (() => void) | null> = { A: null, B: null };

  function clearDeckTimers(deck: 'A' | 'B') {
    if (deckCleanupTimers[deck]) {
      clearTimeout(deckCleanupTimers[deck]!);
      deckCleanupTimers[deck] = null;
    }
    if (deckFadeInCleanups[deck]) {
      deckFadeInCleanups[deck]!();
      deckFadeInCleanups[deck] = null;
    }
  }

  let radioAudioEl: HTMLAudioElement | undefined = $state();
  let radioConfig = $state<RadioHostConfig>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_radio_host') : null;
        if (raw) return JSON.parse(raw);
      } catch {
        // ignore
      }
      return { enabled: false, engine: 'edge', voice: 'male', frequency: 3, city: '' };
    })()
  );
  let radioTrackCount = 0;
  let lastRadioTrackId: string | null = null;
  let radioStatus = $state('');

  function updateRadioConfig(next: RadioHostConfig) {
    radioConfig = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_radio_host', JSON.stringify(next));
    }
  }

  function toAudioVolume(pct: number): number {
    const norm = Math.max(0, Math.min(1, pct / 100));
    return Math.pow(norm, 2.0);
  }

  function calcRadioVoiceVolume(audioVol: number): number {
    return Math.max(0.28, Math.min(0.88, audioVol * 1.65 + 0.12));
  }

  function setMusicDucking(ducked: boolean, factor = 0.26) {
    const targetVol = (ducked ? factor : 1.0) * toAudioVolume(player.volume);
    const cur = getActiveAudio();
    if (cur) cur.volume = Math.max(0, Math.min(1, targetVol));
  }

  async function triggerRadioHost(track: Track, isTest = false) {
    try {
      const res = await api.generateRadioHostLine(
        track,
        radioConfig.engine,
        radioConfig.voice,
        radioConfig.city,
        waveActive
      );
      if (!isTest && player.current_track?.id !== track.id) return;

      if (isTest) {
        radioStatus = res.error
          ? `Fallback (${res.usedEngine}): ${res.error}`
          : `Движок: ${res.usedEngine} — «${res.text}»`;
      }

      setMusicDucking(true);
      const voiceVol = calcRadioVoiceVolume(toAudioVolume(player.volume));
      if (res.audioDataUrl && radioAudioEl) {
        radioAudioEl.src = res.audioDataUrl;
        radioAudioEl.volume = voiceVol;
        radioAudioEl.onended = () => setMusicDucking(false);
        radioAudioEl.onerror = () => setMusicDucking(false);
        await radioAudioEl.play().catch(() => setMusicDucking(false));
      } else if (typeof window !== 'undefined' && 'speechSynthesis' in window) {
        const utter = new SpeechSynthesisUtterance(res.text);
        utter.lang = 'ru-RU';
        utter.volume = voiceVol;
        utter.onend = () => setMusicDucking(false);
        utter.onerror = () => setMusicDucking(false);
        window.speechSynthesis.speak(utter);
      } else {
        setMusicDucking(false);
      }
    } catch {
      setMusicDucking(false);
    }
  }

  function updateMixConfig(next: MixConfig) {
    mixConfig = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_mix_config', JSON.stringify(next));
    }
  }

  let activeDjPreset = $state<MixPreset>('dj-vocal');
  const djPresetPool: MixPreset[] = ['dj-vocal', 'bass-swap', 'rise', 'echo-out', 'dj-vocal', 'fade'];

  function pickDjTransitionPreset(): MixPreset {
    if (mixConfig.mode !== 'auto') {
      activeDjPreset = mixConfig.preset;
      return mixConfig.preset;
    }
    const candidates = djPresetPool.filter((p) => p !== activeDjPreset);
    const chosen = candidates[Math.floor(Math.random() * candidates.length)] ?? 'dj-vocal';
    activeDjPreset = chosen;
    return chosen;
  }

  let waveActive = $state(false);
  let waveTracks = $state<Track[]>([]);
  let waveConfig = $state<WaveConfig>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_wave_config') : null;
        if (raw) return JSON.parse(raw);
      } catch {
        // ignore
      }
      return { sourceFilter: null, mood: 'mix' };
    })()
  );

  function updateWaveConfig(next: WaveConfig) {
    waveConfig = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_wave_config', JSON.stringify(next));
    }
  }

  let effectsSaveTimer: ReturnType<typeof setTimeout> | null = null;

  async function saveEffects(next: AudioEffectsConfig) {
    effects = next;
    audioEngine.applyEffects(next);
    if (effectsSaveTimer) clearTimeout(effectsSaveTimer);
    effectsSaveTimer = setTimeout(async () => {
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('flowly_effects', JSON.stringify(next));
      }
      try {
        await api.updateAudioEffects(next);
      } catch {
        // ignore transient ipc errors during rapid drag
      }
    }, 150);
  }

  function effectiveCrossfadeSec(nextTrack?: Track | null): number {
    if (!mixConfig.enabled) return 0;
    if (waveActive) {
      // В «Моей волне» без Auto-DJ — короткий простой кроссфейд
      return mixConfig.mode === 'manual' ? Math.min(5, Math.max(2, mixConfig.durationSec)) : 3;
    }
    const base = mixConfig.mode === 'auto' ? 6 : mixConfig.durationSec;
    if (mixConfig.mode === 'auto' && player.current_track && nextTrack) {
      return getHarmonicCrossfadeSec(player.current_track, nextTrack, base, true);
    }
    return base;
  }
  let layoutStyle = $state<LayoutStyle>(
    (() => {
      const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_layout_style') : null;
      if (raw === 'studio-split') return 'tv-mode';
      return (raw as LayoutStyle | null) || 'modern-analog';
    })()
  );
  let bgMode = $state<BackgroundMode>(
    (typeof localStorage !== 'undefined' &&
      (localStorage.getItem('ma_bg_mode') as BackgroundMode | null)) ||
      'ambient'
  );
  let activeCssThemeId = $state<string>(
    (typeof localStorage !== 'undefined' && localStorage.getItem('flowly_css_theme_id')) || 'default'
  );
  let customThemeCss = $state<string>(
    (typeof localStorage !== 'undefined' && localStorage.getItem('flowly_custom_css')) || ''
  );

  function applyCssTheme(id: string, css: string) {
    activeCssThemeId = id;
    customThemeCss = css;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_css_theme_id', id);
      localStorage.setItem('flowly_custom_css', css);
    }
  }

  $effect(() => {
    if (typeof document === 'undefined') return;
    let styleEl = document.getElementById('flowly-custom-theme') as HTMLStyleElement | null;
    if (!styleEl) {
      styleEl = document.createElement('style');
      styleEl.id = 'flowly-custom-theme';
      document.head.appendChild(styleEl);
    }
    styleEl.textContent = customThemeCss;
  });

  const layoutOptions: { id: LayoutStyle; label: string; desc: string }[] = [
    { id: 'modern-analog', label: 'Студия', desc: 'Чистая 3-колоночная сцена без лишних панелей' },
    { id: 'wave-dock', label: 'Витрина', desc: 'Классический экран с нижней полкой треков' },
    { id: 'tv-mode', label: 'Кинозал', desc: 'Широкоформатный режим для монитора и ТВ' },
  ];

  function selectLayoutStyle(style: LayoutStyle) {
    layoutStyle = style;
    showLayoutMenu = false;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_layout_style', style);
    }
  }
  let palette = $state<CoverPalette>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_last_palette') : null;
        if (raw) {
          const parsed = JSON.parse(raw);
          if (parsed?.primary && parsed?.secondary && parsed?.tertiary) {
            return {
              primary: parsed.primary,
              secondary: parsed.secondary,
              tertiary: parsed.tertiary,
              quaternary: parsed.quaternary ?? '#ec4899',
              quinary: parsed.quinary ?? '#10b981',
            };
          }
        }
      } catch {
        // ignore
      }
      return {
        primary: '#ff3b5c',
        secondary: '#6366f1',
        tertiary: '#0ea5e9',
        quaternary: '#ec4899',
        quinary: '#10b981',
      };
    })()
  );

  const bgBasicOptions: { id: BackgroundMode; label: string; desc: string }[] = [
    { id: 'ambient', label: 'Ambient Light', desc: 'Мягкое свечение из цветов обложки' },
    { id: 'blur-cover', label: 'Размытая обложка', desc: 'Атмосферный фон из арта трека' },
    { id: 'aurora', label: 'Аврора', desc: 'Плавный живой градиент' },
    { id: 'spotlight', label: 'Студийный свет', desc: 'Сфокусированный верхний прожектор' },
    { id: 'graphite', label: 'Чистый графит', desc: 'Минимализм без лишних эффектов' },
  ];

  const bgReactiveOptions: { id: BackgroundMode; label: string; desc: string }[] = [
    { id: 'rain-glass', label: 'Дождь на стекле', desc: 'Капли дождя реагируют на бас' },
    { id: 'starfield', label: 'Глубокий космос', desc: 'Параллакс-звёзды в цветах обложки' },
    { id: 'fog-grain', label: 'Туманный лес', desc: 'Плёночное зерно и мягкий туман' },
    { id: 'dynamic-lyrics', label: 'Живая типографика', desc: 'Крупный текст песни на фоне' },
  ];

  let bgMenuTab = $state<'basic' | 'reactive'>(
    bgReactiveOptions.some((o) => o.id === bgMode) ? 'reactive' : 'basic'
  );

  function selectBgMode(mode: BackgroundMode) {
    bgMode = mode;
    showBgMenu = false;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('ma_bg_mode', mode);
    }
  }

  let ecoMode = $state(false);
  let ramSaverEnabled = $state<boolean>(
    typeof localStorage !== 'undefined' && localStorage.getItem('flowly_ram_saver') === 'true'
  );

  function updateRamSaver(next: boolean) {
    ramSaverEnabled = next;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_ram_saver', next ? 'true' : 'false');
    }
    api.setTrackCacheEnabled(!next);
    if (next) {
      const inactive = activeDeck === 'A' ? audioB : audioA;
      if (inactive && inactive.paused) {
        inactive.removeAttribute('src');
        inactive.load();
      }
    }
  }
  let currentActivityLabel = $state('');
  let activityConfig = $state<ActivityConfig>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_activity_config') : null;
        if (raw) return JSON.parse(raw);
      } catch {
        // ignore
      }
      return {
        enabled: false,
        codingPlaylistId: '',
        codingEqPreset: 'Lo-Fi',
        codingEco: true,
        gamingEqPreset: 'Gaming',
        gamingEco: true,
        videoAutoPause: true,
        videoAutoResume: true,
        customGames: '',
      };
    })()
  );

  function updateActivityConfig(next: ActivityConfig) {
    activityConfig = next;
    if (!next.enabled) {
      ecoMode = false;
    }
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('flowly_activity_config', JSON.stringify(next));
    }
  }

  const EQ_PRESET_BANDS: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    'Bass Boost': [7.5, 6.5, 5.0, 2.5, 0.5, 0, 0.5, 1.5, 2.0, 2.0],
    Electronic: [5.5, 4.5, 2.0, 0, -1.5, 1.5, 3.0, 4.5, 5.5, 6.0],
    'Lo-Fi': [4.0, 3.5, 2.0, 1.0, 0.5, -1.0, -2.5, -4.0, -5.5, -6.5],
    Gaming: [2.5, 3.0, 0.5, -2.0, -1.5, 1.0, 3.5, 4.5, 3.0, 1.5],
    'Late Night': [3.0, 2.0, 0.5, -1.0, -1.0, 0, 1.0, 1.5, 0.5, -1.0],
  };

  let activeActCategory: ActivityCategory = 'other';
  let pendingActCategory: ActivityCategory = 'other';
  let pendingActCount = 0;
  let pausedByDetector = false;
  let savedEqBeforeActivity: AudioEffectsConfig | null = null;

  async function applyActivityCategory(cat: ActivityCategory) {
    const prev = activeActCategory;
    activeActCategory = cat;

    // Умная пауза и авто-возобновление при просмотре видео
    if (cat === 'video' && activityConfig.videoAutoPause) {
      if (player.is_playing) {
        pausedByDetector = true;
        applyStatePreservingPosition(await api.togglePlayback());
      }
    } else if (prev === 'video' && cat !== 'video' && activityConfig.videoAutoResume && pausedByDetector) {
      pausedByDetector = false;
      if (!player.is_playing) {
        applyStatePreservingPosition(await api.togglePlayback());
      }
    }

    // Eco-режим GPU
    const wantEco =
      (cat === 'coding' && activityConfig.codingEco) ||
      (cat === 'gaming' && activityConfig.gamingEco);
    ecoMode = wantEco;

    // Эквалайзер пресеты
    const targetPreset =
      cat === 'gaming'
        ? activityConfig.gamingEqPreset
        : cat === 'coding'
          ? activityConfig.codingEqPreset
          : '';
    if (targetPreset && EQ_PRESET_BANDS[targetPreset]) {
      if (!savedEqBeforeActivity) {
        savedEqBeforeActivity = { ...effects, bands: [...effects.bands] };
      }
      await saveEffects({
        ...effects,
        eq_enabled: true,
        preset: targetPreset,
        bands: [...EQ_PRESET_BANDS[targetPreset]],
      });
    } else if (savedEqBeforeActivity && cat !== 'coding' && cat !== 'gaming') {
      const restore = savedEqBeforeActivity;
      savedEqBeforeActivity = null;
      await saveEffects(restore);
    }

    // Плейлист для кодинга
    if (cat === 'coding' && activityConfig.codingPlaylistId) {
      const pl = playlists.find((p) => p.id === activityConfig.codingPlaylistId);
      if (pl && pl.tracks.length > 0) {
        const curInPl = player.current_track && pl.tracks.some((t) => t.id === player.current_track?.id);
        if (!curInPl) {
          await handlePlayTrack(pl.tracks[0], pl.tracks);
        }
      }
    }
  }

  let dynamicThemeEnabled = $state(false);
  let weatherCity = $state('');
  let atmosphere = $state<WeatherAtmosphere | null>(null);
  let clockHour = $state(new Date().getHours());

  async function refreshAtmosphere(enabled: boolean, city: string) {
    dynamicThemeEnabled = enabled;
    weatherCity = city;
    clockHour = new Date().getHours();
    if (enabled && city.trim()) {
      try {
        atmosphere = await api.getDynamicAtmosphere(city);
      } catch {
        atmosphere = null;
      }
    } else {
      atmosphere = null;
    }
  }

  const dynamicPalette = $derived.by(() => {
    const h = clockHour;
    const wk = atmosphere?.weatherKind ?? 'cloudy';
    // Утро (6..11)
    if (h >= 6 && h < 12) {
      if (wk === 'clear') return { accent: '#f59e0b', tint: 'rgba(245, 158, 11, 0.14)', sub: 'rgba(251, 113, 133, 0.10)' };
      if (wk === 'rain' || wk === 'storm') return { accent: '#60a5fa', tint: 'rgba(96, 165, 250, 0.15)', sub: 'rgba(148, 163, 184, 0.10)' };
      return { accent: '#38bdf8', tint: 'rgba(56, 189, 248, 0.12)', sub: 'rgba(245, 158, 11, 0.08)' };
    }
    // День (12..17)
    if (h >= 12 && h < 18) {
      if (wk === 'clear') return { accent: '#0ea5e9', tint: 'rgba(14, 165, 233, 0.14)', sub: 'rgba(250, 204, 21, 0.08)' };
      if (wk === 'snow') return { accent: '#7dd3fc', tint: 'rgba(125, 211, 252, 0.15)', sub: 'rgba(224, 242, 254, 0.08)' };
      if (wk === 'rain' || wk === 'storm') return { accent: '#6366f1', tint: 'rgba(99, 102, 241, 0.16)', sub: 'rgba(56, 189, 248, 0.10)' };
      return { accent: '#3b82f6', tint: 'rgba(59, 130, 246, 0.12)', sub: 'rgba(148, 163, 184, 0.08)' };
    }
    // Вечер (18..22)
    if (h >= 18 && h < 23) {
      if (wk === 'clear') return { accent: '#fb7185', tint: 'rgba(251, 113, 133, 0.15)', sub: 'rgba(245, 158, 11, 0.11)' };
      return { accent: '#a855f7', tint: 'rgba(168, 85, 247, 0.15)', sub: 'rgba(59, 130, 246, 0.10)' };
    }
    // Ночь (23..5)
    if (wk === 'rain' || wk === 'storm') return { accent: '#38bdf8', tint: 'rgba(30, 58, 138, 0.22)', sub: 'rgba(56, 189, 248, 0.12)' };
    if (wk === 'snow') return { accent: '#bae6fd', tint: 'rgba(59, 130, 246, 0.16)', sub: 'rgba(186, 230, 253, 0.09)' };
    return { accent: '#818cf8', tint: 'rgba(49, 46, 129, 0.20)', sub: 'rgba(6, 182, 212, 0.10)' };
  });

  const accentColor = $derived(
    dynamicThemeEnabled
      ? dynamicPalette.accent
      : palette.primary || player.current_track?.accentColor || '#ff3b5c'
  );
  const authStatuses = $derived(sources.map((s) => s.auth));
  const anyEnabledServiceConnected = $derived(
    sources.some((s) => enabledSources.includes(s.source) && s.auth.connected),
  );
  const localPlaylists = $derived(playlists.filter((p) => p.isLocal));

  let cloudHydrating = false;
  let cloudSyncTimer: ReturnType<typeof setTimeout> | null = null;

  function buildCloudSettingsPayload(): sb.CloudAccountSettings {
    return {
      layoutStyle,
      bgMode,
      activeCssThemeId,
      customThemeCss,
      ramSaverEnabled,
      effects,
      mixConfig,
      radioConfig,
      activityConfig,
      waveConfig,
      enabledSources,
      dynamicThemeEnabled,
      weatherCity,
    };
  }

  async function applyCloudSettings(data: sb.CloudAccountSettings | null) {
    if (!data) return;
    cloudHydrating = true;
    try {
      if (data.layoutStyle) selectLayoutStyle(data.layoutStyle as LayoutStyle);
      if (data.bgMode) selectBgMode(data.bgMode as BackgroundMode);
      if (typeof data.customThemeCss === 'string') {
        applyCssTheme(data.activeCssThemeId || 'default', data.customThemeCss);
      }
      if (typeof data.ramSaverEnabled === 'boolean') {
        updateRamSaver(data.ramSaverEnabled);
      }
      if (data.effects && typeof data.effects === 'object') {
        await saveEffects(data.effects as AudioEffectsConfig);
      }
      if (data.mixConfig && typeof data.mixConfig === 'object') {
        updateMixConfig(data.mixConfig as MixConfig);
      }
      if (data.radioConfig && typeof data.radioConfig === 'object') {
        updateRadioConfig(data.radioConfig as RadioHostConfig);
      }
      if (data.activityConfig && typeof data.activityConfig === 'object') {
        updateActivityConfig(data.activityConfig as ActivityConfig);
      }
      if (data.waveConfig && typeof data.waveConfig === 'object') {
        updateWaveConfig(data.waveConfig as WaveConfig);
      }
      if (Array.isArray(data.enabledSources) && data.enabledSources.length > 0) {
        updateEnabledSources(data.enabledSources as import('./lib/types').SourceKind[]);
      }
      if (typeof data.dynamicThemeEnabled === 'boolean' || typeof data.weatherCity === 'string') {
        refreshAtmosphere(
          typeof data.dynamicThemeEnabled === 'boolean' ? data.dynamicThemeEnabled : dynamicThemeEnabled,
          typeof data.weatherCity === 'string' ? data.weatherCity : weatherCity
        );
      }
    } finally {
      setTimeout(() => {
        cloudHydrating = false;
      }, 500);
    }
  }

  $effect(() => {
    const sess = sbSession;
    void layoutStyle;
    void bgMode;
    void activeCssThemeId;
    void customThemeCss;
    void ramSaverEnabled;
    void effects;
    void mixConfig;
    void radioConfig;
    void activityConfig;
    void waveConfig;
    void enabledSources;
    void dynamicThemeEnabled;
    void weatherCity;

    if (!sess || cloudHydrating) return;
    if (cloudSyncTimer) clearTimeout(cloudSyncTimer);
    cloudSyncTimer = setTimeout(() => {
      sb.saveCloudSettings(sess, buildCloudSettingsPayload()).catch(() => {});
    }, 2000);
  });

  $effect(() => {
    const tr = player.current_track;
    const playing = player.is_playing;
    if (!tr) return;
    api.updateSmtcState({
      title: tr.title,
      artist: tr.artist,
      album: tr.album || 'Flowly',
      coverUrl: tr.artwork?.startsWith('http') ? tr.artwork : null,
      durationSec: tr.duration || 0,
      playing,
      positionSec: player.position_sec || 0,
    });
    if (typeof navigator !== 'undefined' && 'mediaSession' in navigator) {
      try {
        navigator.mediaSession.metadata = new MediaMetadata({
          title: tr.title,
          artist: tr.artist,
          album: tr.album || 'Flowly',
          artwork: tr.artwork ? [{ src: tr.artwork, sizes: '512x512', type: 'image/jpeg' }] : [],
        });
        navigator.mediaSession.playbackState = playing ? 'playing' : 'paused';
      } catch {
        // ignore
      }
    }
  });

  onMount(async () => {
    if (api.isTauri()) {
      getCurrentWindow().show().catch(() => {});
    }

    try {
      const [st, src, eq, port, savedLikes] = await Promise.all([
        api.getPlayerState(),
        api.getSourcesStatus(),
        api.getAudioEffects(),
        api.getAudioProxyPort(),
        api.getLikedIds(),
      ]);
      player = st;
      sources = src;
      effects = eq;
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('flowly_effects', JSON.stringify(eq));
      }
      audioProxyPort = port;
      likedIds = new Set<string>(savedLikes);
    } catch (e) {
      console.error('Ошибка инициализации:', e);
    } finally {
      const splash = typeof document !== 'undefined' ? document.getElementById('flowly-splash') : null;
      if (splash) {
        splash.classList.add('hide');
        setTimeout(() => splash.remove(), 420);
      }
    }

    api.setTrackCacheEnabled(!ramSaverEnabled);

    if (api.isTauri()) {
      void listen('media:toggle', () => {
        handleToggle();
      });
      void listen('media:play', () => {
        if (!player.is_playing) handleToggle();
      });
      void listen('media:pause', () => {
        if (player.is_playing) handleToggle();
      });
      void listen('media:next', () => {
        handleNext();
      });
      void listen('media:prev', () => {
        handlePrev();
      });
    }
    if (typeof navigator !== 'undefined' && 'mediaSession' in navigator) {
      try {
        navigator.mediaSession.setActionHandler('play', () => { if (!player.is_playing) handleToggle(); });
        navigator.mediaSession.setActionHandler('pause', () => { if (player.is_playing) handleToggle(); });
        navigator.mediaSession.setActionHandler('nexttrack', () => { handleNext(); });
        navigator.mediaSession.setActionHandler('previoustrack', () => { handlePrev(); });
      } catch {
        // ignore
      }
    }

    sb.restoreSession()
      .then(async ({ session, profile }) => {
        sbSession = session;
        sbProfile = profile;
        if (session) {
          const cloud = await sb.loadCloudSettings(session).catch(() => null);
          await applyCloudSettings(cloud);
        }
      })
      .catch(() => {});

    api
      .getNetworkLyricsSettings()
      .then((nl) => {
        refreshAtmosphere(nl.dynamic_theme_enabled, nl.weather_city);
      })
      .catch(() => {});

    const atmosTimer = setInterval(() => {
      clockHour = new Date().getHours();
      if (dynamicThemeEnabled && weatherCity.trim()) {
        refreshAtmosphere(true, weatherCity);
      }
    }, 20 * 60 * 1000);
    void atmosTimer;

    api.scanLocalTracks().then((t) => {
      localTracks = t;
    }).catch(() => {});

    // Восстановление выбранного устройства вывода звука
    try {
      const savedSink = typeof localStorage !== 'undefined' ? localStorage.getItem('flowly_audio_output_id') : null;
      if (savedSink && savedSink !== 'default') {
        void audioEngine.setOutputDevice(savedSink);
      }
    } catch {
      // ignore
    }

    // Проверка, обновилось ли приложение с прошлого запуска
    (async () => {
      try {
        const { getVersion } = await import('@tauri-apps/api/app');
        const currentVersion = await getVersion().catch(() => '0.1.1');
        whatsNewVersion = currentVersion;

        if (typeof localStorage !== 'undefined') {
          const lastSeen = localStorage.getItem('flowly_last_seen_version');
          const pendingRaw = localStorage.getItem('flowly_pending_changelog');

          if (pendingRaw) {
            try {
              const parsed = JSON.parse(pendingRaw);
              if (parsed && parsed.version === currentVersion) {
                whatsNewNotes = parsed.notes ?? '';
                whatsNewDate = parsed.date ?? null;
              }
            } catch {
              // ignore
            }
            localStorage.removeItem('flowly_pending_changelog');
          }

          if (lastSeen && lastSeen !== currentVersion) {
            if (!whatsNewNotes) {
              try {
                const resp = await fetch(
                  `https://api.github.com/repos/whoisquickyyy/flowly/releases/tags/v${currentVersion}`,
                );
                if (resp.ok) {
                  const rel = await resp.json();
                  if (typeof rel?.body === 'string') whatsNewNotes = rel.body.trim();
                  if (typeof rel?.published_at === 'string') whatsNewDate = rel.published_at;
                }
              } catch {
                // ignore offline
              }
            }
            showWhatsNewModal = true;
          }

          localStorage.setItem('flowly_last_seen_version', currentVersion);
        }
      } catch {
        // ignore non-tauri
      }
    })();

    const otaTimer = setTimeout(async () => {
      try {
        const { check } = await import('@tauri-apps/plugin-updater');
        const upd = await check();
        if (upd) {
          availableUpdateVersion = upd.version;
          availableUpdateNotes = upd.body?.trim() ?? '';
          availableUpdateDate = upd.date ?? null;
          showUpdateModal = true;
        }
      } catch {
        // ignore offline / non-tauri
      }
    }, 2000);
    void otaTimer;

    // Детектор активности (опрос раз в 2 сек с debounce 2 тика = ~4 сек)
    const actTimer = setInterval(async () => {
      if (!activityConfig.enabled) return;
      try {
        const customList = activityConfig.customGames
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean);
        const info = await api.getActiveWindowInfo(customList);
        currentActivityLabel = info.appName ? `${info.appName} (${info.category})` : info.category;
        if (info.category === 'flowly') return;
        if (info.category === pendingActCategory) {
          pendingActCount += 1;
        } else {
          pendingActCategory = info.category;
          pendingActCount = 1;
        }
        if (pendingActCount >= 2 && info.category !== activeActCategory) {
          await applyActivityCategory(info.category);
        }
      } catch {
        // ignore
      }
    }, 2000);
    void actTimer;

    // Загружаем сетевую медиатеку в фоне, чтобы не блокировать экран запуска
    Promise.all([
      api.getAllPlaylists().catch(() => []),
      api.getLibrarySection('likedSongs', null).catch(() => []),
      api.getLibrarySection('recentlyPlayed', null).catch(() => []),
    ]).then(([pls, liked, recent]) => {
      playlists = pls;
      likedTracks = liked;
      recentTracks = recent;
      if (liked.length > 0) {
        const merged = new Set<string>(likedIds);
        for (const t of liked) merged.add(t.id);
        likedIds = merged;
      }
    });
  });

  $effect(() => {
    const tr = player.current_track;
    const fallback = tr?.accentColor ?? '#ff3b5c';
    if (!tr?.artwork) {
      palette = {
        primary: fallback,
        secondary: '#a855f7',
        tertiary: '#3b82f6',
        quaternary: '#ec4899',
        quinary: '#10b981',
      };
      return;
    }
    let cancelled = false;
    extractCoverPalette(tr.artwork, fallback, audioProxyPort).then((res) => {
      if (!cancelled) {
        palette = res;
        if (typeof localStorage !== 'undefined') {
          localStorage.setItem('flowly_last_palette', JSON.stringify(res));
        }
      }
    });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    if (!audioA || !audioB) return;
    audioEngine.initDecks(audioA, audioB);
    audioEngine.applyEffects(effects);
  });

  $effect(() => {
    if (!audioA || !audioB) return;
    const rawUrl = player.current_track?.playbackUrl ?? null;
    const srcKind = player.current_track?.source ?? 'yandex';
    const url =
      rawUrl && audioProxyPort > 0 && rawUrl.startsWith('local://')
        ? `http://127.0.0.1:${audioProxyPort}/local?path=${encodeURIComponent(rawUrl.slice(8))}`
        : rawUrl && audioProxyPort > 0 && rawUrl.startsWith('http')
          ? `http://127.0.0.1:${audioProxyPort}/proxy?source=${srcKind}&url=${encodeURIComponent(rawUrl)}`
          : rawUrl;

    if (url !== lastLoadedUrl) {
      const prevDeck = getActiveAudio();
      const naturalEnd = isNaturalEndTransition;
      isNaturalEndTransition = false;

      const xfTargetSec = naturalEnd ? effectiveCrossfadeSec(player.current_track) : 0;
      const manualStyle = mixConfig.manualTransition ?? 'filter-cut';
      const canSwapDecks = Boolean(lastLoadedUrl) && Boolean(url) && Boolean(prevDeck && !prevDeck.paused);
      const shouldCrossfade = canSwapDecks && xfTargetSec > 0;
      const shouldManualTransition = canSwapDecks && !naturalEnd && manualStyle !== 'off';

      const outgoingTrackForMix = lastTrackForMix;
      lastTrackForMix = player.current_track ?? null;
      lastLoadedUrl = url;
      crossfadeTriggeredForId = null;

      if (url) {
        const targetDeck: 'A' | 'B' =
          shouldCrossfade || shouldManualTransition ? (activeDeck === 'A' ? 'B' : 'A') : activeDeck;
        const outgoingDeck: 'A' | 'B' = targetDeck === 'A' ? 'B' : 'A';
        clearDeckTimers(targetDeck);
        activeDeck = targetDeck;
        const incoming = targetDeck === 'A' ? audioA : audioB;
        const outgoing = targetDeck === 'A' ? audioB : audioA;

        if (incoming.src !== url) {
          incoming.src = url;
        }
        incoming.volume = toAudioVolume(player.volume);
        if (outgoing) outgoing.volume = toAudioVolume(player.volume);

        let cleanupSec = 0;
        let deferredOutgoingStop = false;
        if (shouldCrossfade) {
          cleanupSec = xfTargetSec;
          const chosenPreset = waveActive ? 'fade' : pickDjTransitionPreset();
          audioEngine.crossfadeTo(targetDeck, xfTargetSec, chosenPreset, waveActive ? false : mixConfig.eqSweep);
          if (!waveActive && outgoingTrackForMix && player.current_track) {
            const bpmOut = getTrackAudioFeatures(outgoingTrackForMix).bpm;
            const bpmIn = getTrackAudioFeatures(player.current_track).bpm;
            audioEngine.beatmatchTransition(incoming, bpmOut, bpmIn, xfTargetSec);
          } else {
            audioEngine.cancelBeatmatch(incoming);
          }
        } else if (shouldManualTransition) {
          deferredOutgoingStop = true;
          audioEngine.cancelBeatmatch(incoming);
          audioEngine.holdManualSkipIncoming(targetDeck);
          let done = false;
          const runHandoff = () => {
            if (done) return;
            done = true;
            incoming.removeEventListener('playing', runHandoff);
            clearTimeout(fallbackTimer);
            deckFadeInCleanups[targetDeck] = null;
            const outTailSec = audioEngine.handoffManualSkip(targetDeck, manualStyle, outgoing);
            clearDeckTimers(outgoingDeck);
            if (outgoing && !outgoing.paused) {
              if (outTailSec <= 0) {
                outgoing.pause();
                outgoing.removeAttribute('src');
                if (ramSaverEnabled) outgoing.load();
              } else {
                deckCleanupTimers[outgoingDeck] = setTimeout(() => {
                  deckCleanupTimers[outgoingDeck] = null;
                  if (activeDeck === targetDeck && !outgoing.paused) {
                    outgoing.pause();
                    outgoing.removeAttribute('src');
                    if (ramSaverEnabled) outgoing.load();
                  }
                }, (outTailSec + 0.08) * 1000);
              }
            }
          };
          const fallbackTimer = setTimeout(runHandoff, 1200);
          incoming.addEventListener('playing', runHandoff, { once: true });
          deckFadeInCleanups[targetDeck] = () => {
            done = true;
            incoming.removeEventListener('playing', runHandoff);
            clearTimeout(fallbackTimer);
          };
        } else {
          audioEngine.cancelBeatmatch(incoming);
          audioEngine.crossfadeTo(targetDeck, 0, mixConfig.preset, false);
        }

        if (player.is_playing) {
          audioEngine.resume();
          incoming.play().catch(() => {});
          const tid = player.current_track?.id ?? null;
          if (naturalEnd && tid && tid !== lastRadioTrackId) {
            lastRadioTrackId = tid;
            radioTrackCount += 1;
            if (radioConfig.enabled && radioTrackCount % radioConfig.frequency === 0 && player.current_track) {
              triggerRadioHost(player.current_track);
            }
          }
        }
        if (!deferredOutgoingStop) {
          if (cleanupSec <= 0 && outgoing && !outgoing.paused) {
            clearDeckTimers(outgoingDeck);
            outgoing.pause();
            outgoing.removeAttribute('src');
          } else if (cleanupSec > 0 && outgoing) {
            clearDeckTimers(outgoingDeck);
            deckCleanupTimers[outgoingDeck] = setTimeout(() => {
              deckCleanupTimers[outgoingDeck] = null;
              if (activeDeck === targetDeck && !outgoing.paused) {
                outgoing.pause();
                outgoing.removeAttribute('src');
              }
            }, (cleanupSec + 0.15) * 1000);
          }
        }
      } else {
        clearDeckTimers('A');
        clearDeckTimers('B');
        audioA.pause();
        audioA.removeAttribute('src');
        audioB.pause();
        audioB.removeAttribute('src');
      }
    } else if (url) {
      const cur = getActiveAudio();
      if (!cur) return;
      cur.volume = toAudioVolume(player.volume);
      if (player.is_playing && cur.paused) {
        audioEngine.resume();
        cur.play().catch(() => {});
      } else if (!player.is_playing && !cur.paused) {
        cur.pause();
      }
    }
  });

  function applyStatePreservingPosition(next: PlayerState) {
    const hasActiveAudio = Boolean(player.current_track?.playbackUrl && getActiveAudio());
    player = hasActiveAudio ? { ...next, position_sec: player.position_sec } : next;
  }

  async function handlePlayTrack(track: Track, queue?: Track[]) {
    if (jamRoom.connected) {
      if (!jamRoom.isHost) {
        jamRoom.statusText = 'В Jam-комнате треками управляет хост';
        return;
      }
      jamPlayTrack(track);
      return;
    }
    const token = ++switchToken;
    await audioEngine.resume();
    const mStyle = mixConfig.manualTransition ?? 'filter-cut';
    if (player.is_playing && mStyle !== 'off') {
      audioEngine.prepareManualSkipDuck(activeDeck, mStyle);
    }
    crossfadeTriggeredForId = null;
    if (queue !== undefined) {
      waveActive = false;
    }
    try {
      const next = await api.playTrack(track, queue);
      if (token !== switchToken) return;
      player = next;
    } catch (e) {
      audioEngine.restoreDeck(activeDeck);
      console.error('Ошибка запуска трека:', e);
    }
  }

  async function handleStartWave(sourceFilter: import('./lib/types').SourceKind | null) {
    const token = ++switchToken;
    await audioEngine.resume();
    const effectiveFilter =
      sourceFilter ?? (enabledSources.length === 1 ? enabledSources[0] : null);
    try {
      const nextState = await api.startMyWave(effectiveFilter);
      if (token !== switchToken) return;
      player = nextState;
      waveActive = true;
      waveTracks = nextState.queue.filter((t) => enabledSources.includes(t.source));
    } catch (e) {
      console.error('Ошибка запуска волны:', e);
    }
  }

  async function handleResetWave() {
    const token = ++switchToken;
    await audioEngine.resume();
    waveActive = false;
    waveTracks = [];
    const effectiveFilter = enabledSources.length === 1 ? enabledSources[0] : null;
    try {
      const next = await api.resetWaveToLibrary(effectiveFilter);
      if (token !== switchToken) return;
      player = next;
    } catch (e) {
      console.error('Ошибка сброса волны:', e);
    }
  }

  async function handleDislikeTrack(track: Track) {
    if (waveActive) {
      api.reportWaveFeedback(track, false);
    }
    const token = ++switchToken;
    const nextLiked = new Set(likedIds);
    nextLiked.delete(track.id);
    likedIds = nextLiked;
    likedTracks = likedTracks.filter((t) => t.id !== track.id);
    waveTracks = waveTracks.filter((t) => t.id !== track.id);
    try {
      const next = await api.dislikeTrack(track);
      if (token !== switchToken) return;
      player = next;
      if (waveActive) {
        waveTracks = next.queue.filter((t) => enabledSources.includes(t.source));
      }
    } catch (e) {
      console.error('Ошибка дизлайка:', e);
    }
  }

  async function handleToggle() {
    if (jamRoom.connected) {
      if (!jamRoom.isHost) {
        jamRoom.statusText = 'Паузой в Jam-комнате управляет хост';
        return;
      }
      jamTogglePlay();
      return;
    }
    await audioEngine.resume();
    applyStatePreservingPosition(await api.togglePlayback());
  }

  async function handleNext() {
    if (jamRoom.connected) {
      if (jamRoom.isHost && jamRoom.queue.length > 0) {
        jamAdvanceNext();
      } else if (!jamRoom.isHost) {
        jamSkip();
      }
      return;
    }
    if (waveActive && player.current_track) {
      const cur = getActiveAudio();
      const share = cur && cur.duration > 0 ? cur.currentTime / cur.duration : 0;
      const positive = isNaturalEndTransition || share >= 0.7;
      api.reportWaveFeedback(player.current_track, positive);
    }
    const token = ++switchToken;
    const mStyle = mixConfig.manualTransition ?? 'filter-cut';
    if (player.is_playing && !isNaturalEndTransition && mStyle !== 'off') {
      audioEngine.resume();
      audioEngine.prepareManualSkipDuck(activeDeck, mStyle);
    }
    try {
      const prevLen = player.queue.length;
      const next = await api.nextTrack();
      if (token !== switchToken) return;
      player = next;
      if (!waveActive && player.repeat === 'off' && next.queue.length > prevLen) {
        waveActive = true;
      }
      if (waveActive) {
        waveTracks = next.queue;
      }
    } catch (e) {
      audioEngine.restoreDeck(activeDeck);
      console.error('Ошибка переключения вперёд:', e);
    }
  }

  // --- Режим совместного прослушивания (Jam Room / WebSocket Relay) ---
  let jamRoom = $state<JamRoomState>({
    roomCode: '',
    myId: typeof crypto !== 'undefined' && crypto.randomUUID ? crypto.randomUUID() : `u-${Date.now()}`,
    isHost: false,
    connected: false,
    relayUrl: 'supabase',
    members: [],
    queue: [],
    currentTrack: null,
    isPlaying: false,
    positionSec: 0,
    skipVotes: [],
    statusText: '',
  });
  let jamWs: WebSocket | null = null;
  let jamHeartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let jamPhxHeartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let jamPhxRef = 1;
  let jamMemberMap = new Map<string, JamMember>();
  let jamPlayedKeys = new Set<string>();
  let jamRefillBusy = false;
  let jamLastAlignMs = 0;
  let jamLastHostHeartbeatMs = 0;

  async function refillJamQueueIfNeeded() {
    if (!jamRoom.connected || !jamRoom.isHost || jamRefillBusy) return;
    const curIdx = jamRoom.currentTrack
      ? jamRoom.queue.findIndex((q) => q.track.id === jamRoom.currentTrack?.id)
      : -1;
    const remaining = curIdx >= 0 ? jamRoom.queue.length - 1 - curIdx : jamRoom.queue.length;
    if (remaining > 8) return;

    jamRefillBusy = true;
    try {
      const seeds = new Set<string>();
      if (jamRoom.currentTrack?.artist) {
        seeds.add(jamRoom.currentTrack.artist.split(/[,&]/)[0].trim());
      }
      for (const m of jamRoom.members) {
        const sample = m.tracks[Math.floor(Math.random() * Math.max(1, m.tracks.length))];
        if (sample?.artist) seeds.add(sample.artist.split(/[,&]/)[0].trim());
      }
      const existingKeys = new Set(jamRoom.queue.map((q) => trackMatchKey(q.track)));
      const fresh: BlendedTrack[] = [];
      for (const seed of [...seeds].slice(0, 3)) {
        if (!seed) continue;
        const grouped = await api.searchGrouped(seed, null).catch(() => []);
        for (const g of grouped) {
          for (const tr of g.tracks) {
            const k = trackMatchKey(tr);
            if (existingKeys.has(k) || jamPlayedKeys.has(k)) continue;
            existingKeys.add(k);
            fresh.push({
              track: tr,
              reason: `Бесконечный Jam · ${seed}`,
              matchCount: 1,
              score: 50 + Math.random() * 20,
            });
          }
        }
      }
      if (fresh.length > 0) {
        const picked = fresh.sort(() => Math.random() - 0.5).slice(0, 32);
        // Оставляем небольшой хвост истории, но не даём очереди расти бесконечно
        const baseQueue = curIdx > 15 ? jamRoom.queue.slice(curIdx - 5) : jamRoom.queue;
        jamRoom.queue = [...baseQueue, ...picked];
        hostBroadcastState();
      }
    } finally {
      jamRefillBusy = false;
    }
  }

  function jamSendChat(text: string) {
    const clean = text.trim();
    if (!clean || !jamRoom.connected) return;
    const myName =
      jamMemberMap.get(jamRoom.myId)?.name ||
      sbProfile?.username ||
      'Участник';
    const msg: JamChatMessage = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      senderId: jamRoom.myId,
      senderName: myName,
      text: clean,
      ts: Date.now(),
    };
    jamRoom.chat = [...(jamRoom.chat ?? []).slice(-49), msg];
    sendJamMsg({ type: 'chat', message: msg });
  }

  function sendJamMsg(msg: Record<string, unknown>) {
    if (!jamWs || jamWs.readyState !== WebSocket.OPEN) return;
    if (jamRoom.relayUrl === 'supabase') {
      jamWs.send(
        JSON.stringify({
          topic: `realtime:jam-${jamRoom.roomCode}`,
          event: 'broadcast',
          payload: {
            type: 'broadcast',
            event: 'jam_msg',
            payload: msg,
          },
          ref: String(jamPhxRef++),
        })
      );
    } else {
      jamWs.send(JSON.stringify(msg));
    }
  }

  async function playRoomTrackLocally(track: Track, shouldPlay: boolean, posSec = 0) {
    await audioEngine.resume();
    const queueTracks = jamRoom.queue.map((q) => q.track);
    let targetTrack = track;
    const isSrcConnected = sources.some((s) => s.source === track.source && s.auth.connected);
    if (!isSrcConnected) {
      try {
        const res = await api.searchGrouped(`${track.artist} ${track.title}`, null);
        const alt = res.flatMap((g) => g.tracks)[0];
        if (alt) targetTrack = alt;
      } catch {
        // fallback to original track
      }
    }
    waveActive = false;
    player = await api.playTrack(targetTrack, queueTracks.length > 0 ? queueTracks : [targetTrack]);
    if (!shouldPlay && player.is_playing) {
      applyStatePreservingPosition(await api.togglePlayback());
    }
    if (posSec > 2) {
      const cur = getActiveAudio();
      if (cur) cur.currentTime = posSec;
      player.position_sec = Math.floor(posSec);
    }
  }

  function hostBroadcastState() {
    if (!jamRoom.isHost) return;
    sendJamMsg({
      type: 'room_state',
      members: jamRoom.members,
      queue: jamRoom.queue,
      currentTrack: jamRoom.currentTrack,
      isPlaying: jamRoom.isPlaying,
      positionSec: player.position_sec,
      skipVotes: jamRoom.skipVotes,
      sentAtMs: Date.now(),
    });
  }

  function jamRebuildBlend() {
    const members = [...jamMemberMap.values()];
    const blended = buildBlendedQueue(members);
    jamRoom.members = members;
    jamRoom.queue = blended;
    if (jamRoom.isHost) {
      hostBroadcastState();
    } else {
      sendJamMsg({ type: 'request_rebuild' });
    }
  }

  function jamAdvanceNext() {
    if (jamRoom.queue.length === 0) return;
    const curIdx = jamRoom.currentTrack
      ? jamRoom.queue.findIndex((q) => q.track.id === jamRoom.currentTrack?.id)
      : -1;
    const nextItem = jamRoom.queue[(curIdx + 1) % jamRoom.queue.length];
    if (!nextItem) return;
    if (jamRoom.currentTrack) {
      jamPlayedKeys.add(trackMatchKey(jamRoom.currentTrack));
    }
    jamPlayedKeys.add(trackMatchKey(nextItem.track));
    if (jamPlayedKeys.size > 400) jamPlayedKeys.clear();
    jamRoom.currentTrack = nextItem.track;
    jamRoom.isPlaying = true;
    jamRoom.positionSec = 0;
    jamRoom.skipVotes = [];
    playRoomTrackLocally(nextItem.track, true, 0);
    sendJamMsg({
      type: 'sync_play',
      track: nextItem.track,
      isPlaying: true,
      positionSec: 0,
      sentAtMs: Date.now(),
    });
    void refillJamQueueIfNeeded();
  }

  function openJamSocket(
    code: string,
    isHost: boolean,
    name: string,
    relayUrl: string,
    tasteLabel: string,
    tracks: Track[]
  ) {
    leaveJamRoom();
    const myMember: JamMember = {
      id: jamRoom.myId,
      name,
      avatarUrl: sbProfile?.avatar_url || undefined,
      tag: sbProfile?.tag || undefined,
      role: sbProfile?.role,
      isHost,
      tasteLabel,
      trackCount: tracks.length,
      tracks,
    };
    jamMemberMap = new Map([[myMember.id, myMember]]);
    const initialQueue = isHost ? buildBlendedQueue([myMember]) : [];

    jamRoom = {
      ...jamRoom,
      roomCode: code,
      isHost,
      connected: false,
      relayUrl,
      members: [myMember],
      queue: initialQueue,
      currentTrack: isHost ? initialQueue[0]?.track ?? null : null,
      isPlaying: false,
      positionSec: 0,
      skipVotes: [],
      statusText: relayUrl === 'supabase' ? 'Подключение к Supabase Realtime...' : 'Подключение к Relay...',
    };

    const isSb = relayUrl === 'supabase';
    const wsUrl = isSb
      ? sb.getSupabaseRealtimeWsUrl()
      : `${relayUrl}${relayUrl.includes('?') ? '&' : '?'}room=${encodeURIComponent(code)}`;
    const ws = new WebSocket(wsUrl);
    jamWs = ws;

    ws.onopen = () => {
      if (isSb) {
        ws.send(
          JSON.stringify({
            topic: `realtime:jam-${code}`,
            event: 'phx_join',
            payload: { config: { broadcast: { self: false } } },
            ref: String(jamPhxRef++),
          })
        );
        if (jamPhxHeartbeatTimer === null) {
          jamPhxHeartbeatTimer = setInterval(() => {
            if (jamWs && jamWs.readyState === WebSocket.OPEN) {
              jamWs.send(
                JSON.stringify({
                  topic: 'phoenix',
                  event: 'heartbeat',
                  payload: {},
                  ref: String(jamPhxRef++),
                })
              );
            }
          }, 25_000);
        }
      }
      jamRoom.connected = true;
      jamRoom.statusText = isHost ? 'Комната создана' : 'Подключено к комнате';
      setTimeout(() => sendJamMsg({ type: 'join', member: myMember }), isSb ? 250 : 0);
      if (isHost && jamHeartbeatTimer === null) {
        jamHeartbeatTimer = setInterval(() => {
          if (jamRoom.connected && jamRoom.isHost && jamRoom.currentTrack) {
            sendJamMsg({
              type: 'heartbeat',
              trackId: jamRoom.currentTrack.id,
              positionSec: player.position_sec,
              isPlaying: player.is_playing,
              sentAtMs: Date.now(),
            });
          }
        }, 3000);
      }
    };

    ws.onmessage = async (ev) => {
      try {
        const raw = JSON.parse(ev.data);
        const msg = isSb
          ? raw.event === 'broadcast'
            ? raw.payload?.payload
            : null
          : raw;
        if (!msg) return;
        if (msg.type === 'join' && msg.member) {
          const m = msg.member as JamMember;
          jamMemberMap.set(m.id, m);
          if (jamRoom.isHost) {
            jamRoom.members = [...jamMemberMap.values()];
            jamRoom.queue = buildBlendedQueue(jamRoom.members);
            if (!jamRoom.currentTrack && jamRoom.queue[0]) {
              jamRoom.currentTrack = jamRoom.queue[0].track;
            }
            hostBroadcastState();
          }
        } else if (msg.type === 'update_taste' && msg.member) {
          const m = msg.member as JamMember;
          jamMemberMap.set(m.id, m);
          if (jamRoom.isHost) {
            jamRebuildBlend();
          }
        } else if (msg.type === 'leave' && msg.memberId) {
          jamMemberMap.delete(msg.memberId);
          if (jamRoom.isHost) {
            jamRoom.members = [...jamMemberMap.values()];
            jamRoom.queue = buildBlendedQueue(jamRoom.members);
            jamRoom.skipVotes = jamRoom.skipVotes.filter((id) => id !== msg.memberId);
            hostBroadcastState();
          }
        } else if (msg.type === 'room_state') {
          jamRoom.members = msg.members ?? [];
          jamMemberMap = new Map(jamRoom.members.map((m: JamMember) => [m.id, m]));
          jamRoom.queue = msg.queue ?? [];
          jamRoom.skipVotes = msg.skipVotes ?? [];
          const incomingTrack = msg.currentTrack as Track | null;
          const driftPos =
            (msg.positionSec ?? 0) + (msg.isPlaying ? Math.max(0, (Date.now() - (msg.sentAtMs ?? Date.now())) / 1000) : 0);
          if (incomingTrack && incomingTrack.id !== jamRoom.currentTrack?.id) {
            jamRoom.currentTrack = incomingTrack;
            jamRoom.isPlaying = Boolean(msg.isPlaying);
            await playRoomTrackLocally(incomingTrack, Boolean(msg.isPlaying), driftPos);
          }
        } else if (msg.type === 'sync_play' && msg.track) {
          const tr = msg.track as Track;
          jamRoom.currentTrack = tr;
          jamRoom.isPlaying = Boolean(msg.isPlaying);
          jamRoom.skipVotes = [];
          await playRoomTrackLocally(tr, Boolean(msg.isPlaying), msg.positionSec ?? 0);
        } else if (msg.type === 'sync_toggle') {
          jamRoom.isPlaying = Boolean(msg.isPlaying);
          if (player.is_playing !== jamRoom.isPlaying) {
            applyStatePreservingPosition(await api.togglePlayback());
          }
        } else if (msg.type === 'vote_skip' && msg.memberId) {
          if (!jamRoom.skipVotes.includes(msg.memberId)) {
            jamRoom.skipVotes = [...jamRoom.skipVotes, msg.memberId];
          }
          const needed = Math.max(1, Math.ceil(jamRoom.members.length / 2));
          if (jamRoom.isHost && jamRoom.skipVotes.length >= needed) {
            jamAdvanceNext();
          }
        } else if (msg.type === 'request_rebuild' && jamRoom.isHost) {
          jamRebuildBlend();
        } else if (msg.type === 'chat' && msg.message) {
          const chatMsg = msg.message as JamChatMessage;
          jamRoom.chat = [...(jamRoom.chat ?? []).slice(-49), chatMsg];
        } else if (msg.type === 'heartbeat' && !jamRoom.isHost) {
          jamLastHostHeartbeatMs = Date.now();
          if (msg.trackId && jamRoom.currentTrack?.id === msg.trackId) {
            if (player.is_playing !== Boolean(msg.isPlaying)) {
              jamRoom.isPlaying = Boolean(msg.isPlaying);
              applyStatePreservingPosition(await api.togglePlayback());
            }
            const expected =
              (msg.positionSec ?? 0) +
              (msg.isPlaying ? Math.max(0, (Date.now() - (msg.sentAtMs ?? Date.now())) / 1000) : 0);
            const drift = Math.abs(player.position_sec - expected);
            if (drift > 0.45) {
              jamRoom.syncStatus = 'catching-up';
              if (Date.now() - jamLastAlignMs > 2500) {
                jamLastAlignMs = Date.now();
                const cur = getActiveAudio();
                if (cur) cur.currentTime = expected;
                player.position_sec = Math.floor(expected);
              }
            } else {
              jamRoom.syncStatus = 'synced';
            }
          }
        }
      } catch {
        // ignore parse errors
      }
    };

    ws.onerror = () => {
      jamRoom.statusText = 'Ошибка подключения к Relay. Проверьте URL или запустите relay/server.mjs';
    };

    ws.onclose = () => {
      jamRoom.connected = false;
      if (jamHeartbeatTimer) {
        clearInterval(jamHeartbeatTimer);
        jamHeartbeatTimer = null;
      }
      if (jamPhxHeartbeatTimer) {
        clearInterval(jamPhxHeartbeatTimer);
        jamPhxHeartbeatTimer = null;
      }
    };
  }

  function createJamRoom(name: string, relayUrl: string, tasteLabel: string, tracks: Track[]) {
    openJamSocket(generateRoomCode(), true, name, relayUrl, tasteLabel, tracks);
  }

  function joinJamRoom(code: string, name: string, relayUrl: string, tasteLabel: string, tracks: Track[]) {
    openJamSocket(code, false, name, relayUrl, tasteLabel, tracks);
  }

  function leaveJamRoom() {
    if (jamWs) {
      sendJamMsg({ type: 'leave', memberId: jamRoom.myId });
      jamWs.close();
      jamWs = null;
    }
    if (jamHeartbeatTimer) {
      clearInterval(jamHeartbeatTimer);
      jamHeartbeatTimer = null;
    }
    if (jamPhxHeartbeatTimer) {
      clearInterval(jamPhxHeartbeatTimer);
      jamPhxHeartbeatTimer = null;
    }
    jamRoom.connected = false;
    jamRoom.members = [];
    jamRoom.queue = [];
    jamRoom.skipVotes = [];
  }

  function updateJamTaste(tasteLabel: string, tracks: Track[]) {
    const me = jamMemberMap.get(jamRoom.myId);
    if (!me) return;
    const updated: JamMember = { ...me, tasteLabel, trackCount: tracks.length, tracks };
    jamMemberMap.set(jamRoom.myId, updated);
    sendJamMsg({ type: 'update_taste', member: updated });
    if (jamRoom.isHost) {
      jamRebuildBlend();
    }
  }

  function jamPlayTrack(track: Track) {
    jamRoom.currentTrack = track;
    jamRoom.isPlaying = true;
    jamRoom.skipVotes = [];
    playRoomTrackLocally(track, true, 0);
    sendJamMsg({
      type: 'sync_play',
      track,
      isPlaying: true,
      positionSec: 0,
      sentAtMs: Date.now(),
    });
  }

  async function jamTogglePlay() {
    await handleToggle();
    jamRoom.isPlaying = player.is_playing;
    sendJamMsg({ type: 'sync_toggle', isPlaying: player.is_playing });
  }

  function jamSkip() {
    if (jamRoom.isHost) {
      // Хост может скипать мгновенно без опроса
      jamAdvanceNext();
      return;
    }
    if (!jamRoom.skipVotes.includes(jamRoom.myId)) {
      jamRoom.skipVotes = [...jamRoom.skipVotes, jamRoom.myId];
    }
    sendJamMsg({ type: 'vote_skip', memberId: jamRoom.myId });
  }

  async function cyclePlaybackMode() {
    if (waveActive) {
      applyStatePreservingPosition(await api.cycleRepeat());
      return;
    }
    if (!player.shuffle && player.repeat === 'off') {
      applyStatePreservingPosition(await api.cycleRepeat());
    } else if (!player.shuffle && player.repeat === 'all') {
      applyStatePreservingPosition(await api.cycleRepeat());
    } else if (!player.shuffle && player.repeat === 'one') {
      await api.cycleRepeat();
      applyStatePreservingPosition(await api.toggleShuffle());
    } else {
      let st = await api.toggleShuffle();
      while (st.repeat !== 'off') {
        st = await api.cycleRepeat();
      }
      applyStatePreservingPosition(st);
    }
  }

  const playbackModeInfo = $derived.by(() => {
    if (player.shuffle) return { icon: 'shuffle-bold', label: 'Шафл', active: true };
    if (player.repeat === 'one') return { icon: 'repeat-one-bold', label: 'Повтор 1', active: true };
    if (player.repeat === 'all') return { icon: 'repeat-bold', label: 'Повтор', active: true };
    return { icon: 'repeat-linear', label: 'Порядок', active: false };
  });

  async function handlePrev() {
    if (jamRoom.connected && !jamRoom.isHost) {
      jamRoom.statusText = 'В Jam-комнате переключением управляет хост';
      return;
    }
    const token = ++switchToken;
    const mStyle = mixConfig.manualTransition ?? 'filter-cut';
    if (player.is_playing && mStyle !== 'off') {
      audioEngine.resume();
      audioEngine.prepareManualSkipDuck(activeDeck, mStyle);
    }
    try {
      const next = await api.prevTrack();
      if (token !== switchToken) return;
      player = next;
    } catch (e) {
      audioEngine.restoreDeck(activeDeck);
      console.error('Ошибка переключения назад:', e);
    }
  }

  async function handleTrackEnded(deck: 'A' | 'B') {
    if (deck !== activeDeck) return;
    const cur = getActiveAudio();
    if (player.repeat === 'one' && cur) {
      recapSession = {
        trackId: player.current_track?.id ?? null,
        lastTickMs: Date.now(),
        listenedSec: 0,
        recorded: false,
      };
      cur.currentTime = 0;
      player.position_sec = 0;
      cur.play().catch(() => {});
      return;
    }
    if (crossfadeTriggeredForId === player.current_track?.id) return;
    isNaturalEndTransition = true;
    await handleNext();
  }

  let recapSession = {
    trackId: null as string | null,
    lastTickMs: 0,
    listenedSec: 0,
    recorded: false,
  };

  function handleDeckTimeUpdate(deck: 'A' | 'B', el: HTMLAudioElement) {
    if (deck !== activeDeck || Number.isNaN(el.currentTime)) return;
    const nextSec = Math.floor(el.currentTime);
    if (player.position_sec !== nextSec) {
      player.position_sec = nextSec;
    }

    // Трекинг честных прослушиваний для вкладки «Итоги»
    const curTrack = player.current_track;
    if (curTrack && player.is_playing && !el.paused) {
      const now = Date.now();
      if (recapSession.trackId !== curTrack.id) {
        recapSession = { trackId: curTrack.id, lastTickMs: now, listenedSec: 0, recorded: false };
      } else {
        const dt = Math.min(2, Math.max(0, (now - recapSession.lastTickMs) / 1000));
        recapSession.lastTickMs = now;
        recapSession.listenedSec += dt;
        const threshold = Math.min(25, Math.max(8, Math.floor((curTrack.duration || 60) * 0.35)));
        if (!recapSession.recorded && recapSession.listenedSec >= threshold) {
          recapSession.recorded = true;
          const creditedSec = curTrack.duration > 0 ? curTrack.duration : Math.round(recapSession.listenedSec);
          api.recordListen(curTrack, creditedSec).catch(() => {});
        }
      }
    }

    // Предзагрузка следующего трека в свободную деку за 14 секунд до конца (отключается в режиме экономии ОЗУ)
    const trackId = player.current_track?.id ?? null;
    if (
      !ramSaverEnabled &&
      player.is_playing &&
      player.repeat !== 'one' &&
      trackId &&
      preloadedForTrackId !== trackId &&
      Number.isFinite(el.duration) &&
      el.duration > 18 &&
      el.duration - el.currentTime <= 14
    ) {
      preloadedForTrackId = trackId;
      api
        .preloadNextTrack()
        .then((nextTr) => {
          if (!nextTr?.playbackUrl || preloadedForTrackId !== trackId) return;
          const inactive = activeDeck === 'A' ? audioB : audioA;
          if (!inactive || !inactive.paused) return;
          const nextProxy =
            audioProxyPort > 0 && nextTr.playbackUrl.startsWith('local://')
              ? `http://127.0.0.1:${audioProxyPort}/local?path=${encodeURIComponent(nextTr.playbackUrl.slice(8))}`
              : audioProxyPort > 0 && nextTr.playbackUrl.startsWith('http')
                ? `http://127.0.0.1:${audioProxyPort}/proxy?source=${nextTr.source}&url=${encodeURIComponent(nextTr.playbackUrl)}`
                : nextTr.playbackUrl;
          inactive.preload = 'auto';
          inactive.src = nextProxy;
        })
        .catch(() => {});
    }

    // Auto-Mix / Crossfade за N секунд до конца трека
    const xf = effectiveCrossfadeSec();
    const expectedDur = player.current_track?.duration ?? 0;
    if (
      xf > 0 &&
      player.is_playing &&
      player.repeat !== 'one' &&
      trackId &&
      crossfadeTriggeredForId !== trackId &&
      Number.isFinite(el.duration) &&
      el.duration >= Math.max(xf + 4, expectedDur * 0.5) &&
      el.duration - el.currentTime <= xf
    ) {
      crossfadeTriggeredForId = trackId;
      isNaturalEndTransition = true;
      handleNext();
    }
  }

  let seekSaveTimer: ReturnType<typeof setTimeout> | null = null;

  async function handleSeekVal(val: number) {
    const cur = getActiveAudio();
    if (cur && player.current_track?.playbackUrl) {
      cur.currentTime = val;
    }
    const sec = Math.max(0, Math.floor(val));
    if (player.position_sec !== sec) {
      player.position_sec = sec;
    }
    if (seekSaveTimer) clearTimeout(seekSaveTimer);
    seekSaveTimer = setTimeout(async () => {
      try {
        const next = await api.seekTo(val);
        player.position_sec = next.position_sec;
      } catch {
        // ignore transient ipc errors during rapid drag
      }
    }, 120);
  }

  let volumeSaveTimer: ReturnType<typeof setTimeout> | null = null;

  function handleVolumeVal(val: number) {
    const clamped = Math.max(0, Math.min(100, Math.round(val)));
    const audioVol = toAudioVolume(clamped);
    player.volume = clamped;
    if (audioA) audioA.volume = audioVol;
    if (audioB) audioB.volume = audioVol;
    if (radioAudioEl && !radioAudioEl.paused) {
      radioAudioEl.volume = calcRadioVoiceVolume(audioVol);
    }
    audioEngine.resume();

    if (volumeSaveTimer) clearTimeout(volumeSaveTimer);
    volumeSaveTimer = setTimeout(() => {
      api.setVolume(clamped).catch(() => {});
    }, 100);
  }

  async function handleVolume(e: Event) {
    const val = Number((e.target as HTMLInputElement).value);
    await handleVolumeVal(val);
  }

  async function handleCreatePlaylist(title: string) {
    playlists = await api.createLocalPlaylist(title, null);
  }

  async function handleAddToPlaylist(playlistId: string, track: Track) {
    playlists = await api.addTrackToPlaylist(playlistId, track);
  }

  async function handleRemoveFromPlaylist(playlistId: string, trackId: string) {
    playlists = await api.removeTrackFromPlaylist(playlistId, trackId);
  }

  async function handleToggleLike(track: Track) {
    const nextLiked = !likedIds.has(track.id);
    const nextSet = new Set(likedIds);
    if (nextLiked) {
      nextSet.add(track.id);
      if (!likedTracks.some((t) => t.id === track.id)) {
        likedTracks = [track, ...likedTracks];
      }
    } else {
      nextSet.delete(track.id);
      likedTracks = likedTracks.filter((t) => t.id !== track.id);
    }
    likedIds = nextSet;
    try {
      const ids = await api.toggleLikeTrack(track, nextLiked);
      const updated = new Set(ids);
      for (const t of likedTracks) updated.add(t.id);
      if (!nextLiked) updated.delete(track.id);
      likedIds = updated;
    } catch {
      // keep optimistic state
    }
  }

  let mainAreaEl: HTMLElement | undefined = $state();
  const tabScrolls: Record<string, number> = {};

  function saveCurrentScroll() {
    if (!mainAreaEl) return;
    const key = activeArtist ? `artist:${activeArtist}` : activeTab;
    tabScrolls[key] = mainAreaEl.scrollTop;
  }

  function restoreScroll(key: string) {
    requestAnimationFrame(() => {
      if (mainAreaEl) {
        mainAreaEl.scrollTop = tabScrolls[key] ?? 0;
      }
    });
  }

  function goTab(tab: TabId) {
    saveCurrentScroll();
    activeArtist = null;
    activeTab = tab;
    restoreScroll(tab);
    if (tab === 'playlists' && playlists.length === 0) {
      api.getAllPlaylists().then((pls) => {
        if (pls.length > 0) playlists = pls;
      }).catch(() => {});
    }
  }

  function handleOpenArtist(artist: string) {
    saveCurrentScroll();
    activeArtist = artist;
    restoreScroll(`artist:${artist}`);
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      const input = document.querySelector<HTMLInputElement>('.top-search input');
      input?.focus();
      input?.select();
      return;
    }
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    if (e.code === 'Space') {
      e.preventDefault();
      handleToggle();
    }
  }

  function formatTime(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<audio
  bind:this={audioA}
  crossorigin="anonymous"
  ontimeupdate={() => audioA && handleDeckTimeUpdate('A', audioA)}
  onended={() => handleTrackEnded('A')}
></audio>
<audio
  bind:this={audioB}
  crossorigin="anonymous"
  ontimeupdate={() => audioB && handleDeckTimeUpdate('B', audioB)}
  onended={() => handleTrackEnded('B')}
></audio>
<audio bind:this={radioAudioEl}></audio>

<div
  class="shell"
  style="--accent: {accentColor}; --ambient-1: {palette.primary}; --ambient-2: {palette.secondary}; --ambient-3: {palette.tertiary}; --ambient-4: {palette.quaternary}; --ambient-5: {palette.quinary}"
>
  {#if bgMode === 'ambient'}
    <div class="bg-layer bg-ambient" class:playing={player.is_playing}></div>
    <div class="bg-layer bg-ambient-orb" class:playing={player.is_playing}></div>
  {:else if bgMode === 'blur-cover'}
    {#if player.current_track?.artwork}
      <div
        class="bg-layer bg-blur-cover"
        style="background-image: url('{player.current_track.artwork}')"
      ></div>
      <div class="bg-layer bg-blur-scrim"></div>
    {/if}
  {:else if bgMode === 'aurora'}
    <div class="bg-layer bg-aurora" class:playing={player.is_playing}></div>
  {:else if bgMode === 'spotlight'}
    <div class="bg-layer bg-spotlight"></div>
  {:else if bgMode !== 'graphite' && !ecoMode && !ramSaverEnabled && (bgMode !== 'dynamic-lyrics' || (!activeArtist && activeTab === 'details'))}
    <ReactiveBackground
      mode={bgMode}
      track={player.current_track}
      {palette}
      isPlaying={player.is_playing}
      positionSec={player.position_sec}
      proxyPort={audioProxyPort}
    />
  {/if}
  {#if dynamicThemeEnabled}
    <div
      class="bg-layer bg-dynamic-tint"
      style={`--dyn-tint: ${dynamicPalette.tint}; --dyn-sub: ${dynamicPalette.sub}`}
    ></div>
    {#if !ecoMode && !ramSaverEnabled}
      <WeatherAtmosphereLayer {atmosphere} hour={clockHour} />
    {/if}
  {/if}

  <!-- Верхняя панель -->
  <header class="topbar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <FlowlyLogo size={20} />
      <span class="brand-name">Flowly</span>
      {#if availableUpdateVersion}
        <button
          type="button"
          class="ota-pill"
          onclick={() => (showUpdateModal = true)}
          title="Посмотреть список изменений и установить обновление"
        >
          {installingOta ? 'Обновление...' : `Обновить до v${availableUpdateVersion}`}
        </button>
      {/if}
    </div>

    <form
      class="top-search"
      onsubmit={(e) => {
        e.preventDefault();
        activeTab = 'search';
      }}
    >
      <Search size={14} />
      <input
        type="text"
        bind:value={quickSearch}
        placeholder="Что хочешь послушать?"
        onfocus={() => {
          if (quickSearch.trim()) activeTab = 'search';
        }}
      />
    </form>

    <div class="win-controls">
      <div class="bg-picker-wrap">
        <button
          type="button"
          class="win-btn bg-btn"
          class:active={showLayoutMenu}
          onclick={() => {
            showLayoutMenu = !showLayoutMenu;
            showBgMenu = false;
          }}
          title="Стиль плеера"
          aria-label="Стиль плеера"
        >
          <LayoutGrid size={13} />
        </button>
        {#if showLayoutMenu}
          <div class="bg-popover">
            <div class="bg-pop-title">Макет плеера</div>
            {#each layoutOptions as opt (opt.id)}
              <button
                type="button"
                class="bg-opt"
                class:active={layoutStyle === opt.id}
                onclick={() => selectLayoutStyle(opt.id)}
              >
                <span class="bg-opt-name">{opt.label}</span>
                <span class="bg-opt-desc">{opt.desc}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <div class="bg-picker-wrap">
        <button
          type="button"
          class="win-btn bg-btn"
          class:active={showBgMenu}
          onclick={() => {
            showBgMenu = !showBgMenu;
            showLayoutMenu = false;
          }}
          title="Выбор фона"
          aria-label="Выбор фона"
        >
          <Palette size={13} />
        </button>
        {#if showBgMenu}
          <div class="bg-popover">
            <div class="bg-pop-tabs">
              <button
                type="button"
                class:active={bgMenuTab === 'basic'}
                onclick={() => (bgMenuTab = 'basic')}
              >
                Базовые
              </button>
              <button
                type="button"
                class:active={bgMenuTab === 'reactive'}
                onclick={() => (bgMenuTab = 'reactive')}
              >
                Реактивные
              </button>
            </div>
            {#each bgMenuTab === 'basic' ? bgBasicOptions : bgReactiveOptions as opt (opt.id)}
              <button
                type="button"
                class="bg-opt"
                class:active={bgMode === opt.id}
                onclick={() => selectBgMode(opt.id)}
              >
                <span class="bg-opt-name">{opt.label}</span>
                <span class="bg-opt-desc">{opt.desc}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="win-btn" onclick={() => api.isTauri() && getCurrentWindow().minimize()} aria-label="Свернуть">
        <Minus size={14} />
      </button>
      <button class="win-btn" onclick={() => api.isTauri() && getCurrentWindow().toggleMaximize()} aria-label="Развернуть">
        <Square size={11} />
      </button>
      <button class="win-btn close" onclick={() => api.isTauri() && getCurrentWindow().close()} aria-label="Закрыть">
        <X size={14} />
      </button>
    </div>
  </header>

  <!-- Тело: узкий icon-rail слева + основная область -->
  <div class="body-frame">
    <nav class="icon-rail" aria-label="Основная навигация">
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'details'}
        onclick={() => goTab('details')}
        title="Сейчас играет"
      >
        <Disc3 size={18} />
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'library'}
        onclick={() => goTab('library')}
        title="Библиотека"
      >
        <Library size={18} />
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'search'}
        onclick={() => goTab('search')}
        title="Поиск"
      >
        <Search size={18} />
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'playlists'}
        onclick={() => goTab('playlists')}
        title="Плейлисты"
      >
        <ListMusic size={18} />
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'recap'}
        onclick={() => goTab('recap')}
        title="Итоги дня / недели / месяца"
      >
        <BarChart3 size={18} />
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'room'}
        onclick={() => goTab('room')}
        title="Общая комната (Jam)"
      >
        <Radio size={18} />
        {#if jamRoom.connected}
          <span class="rail-dot"></span>
        {/if}
      </button>
      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'equalizer'}
        onclick={() => goTab('equalizer')}
        title="Эквалайзер и DSP"
      >
        <Sliders size={18} />
      </button>

      <div class="rail-spacer"></div>

      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'settings' && settingsInitialTab === 'account'}
        onclick={() => {
          settingsInitialTab = 'account';
          goTab('settings');
        }}
        title={sbProfile ? `${sbProfile.username}#${sbProfile.tag}` : 'Профиль Flowly'}
      >
        {#if sbProfile?.avatar_url}
          <img class="rail-avatar" src={sbProfile.avatar_url} alt={sbProfile.username} />
        {:else if sbProfile}
          <span class="rail-initial">{sbProfile.username.slice(0, 1).toUpperCase()}</span>
        {:else}
          <User size={18} />
        {/if}
      </button>

      <button
        type="button"
        class="rail-btn"
        class:active={!activeArtist && activeTab === 'settings' && settingsInitialTab !== 'account'}
        onclick={() => {
          if (settingsInitialTab === 'account') settingsInitialTab = 'services';
          goTab('settings');
        }}
        title="Настройки и сервисы"
      >
        <Settings size={18} />
      </button>
    </nav>

    <main class="main-area" class:has-floating-bar={activeTab !== 'details'} bind:this={mainAreaEl}>
      {#if sources.length > 0 && !anyEnabledServiceConnected && activeTab !== 'settings'}
        <div class="global-unauth-banner">
          <div class="global-unauth-copy">
            <strong>Вы не авторизованы</strong>
            <span>Подключите Яндекс Музыку или SoundCloud, чтобы слушать треки, искать музыку и запускать «Мою волну».</span>
          </div>
          <button
            type="button"
            class="global-unauth-btn"
            onclick={() => {
              settingsInitialTab = 'services';
              goTab('settings');
            }}
          >
            Войти в сервис
          </button>
        </div>
      {/if}

      {#if activeArtist}
        <ArtistView
          artistName={activeArtist}
          currentTrackId={player.current_track?.id ?? null}
          {likedIds}
          onBack={() => {
            saveCurrentScroll();
            activeArtist = null;
            restoreScroll(activeTab);
          }}
          onPlayTrack={handlePlayTrack}
          onToggleLike={handleToggleLike}
        />
      {/if}

      <div class="tab-keep" class:tab-hidden={Boolean(activeArtist) || activeTab !== 'library'}>
        <LibraryView
          currentTrackId={player.current_track?.id ?? null}
          {localPlaylists}
          {likedIds}
          {enabledSources}
          {authStatuses}
          onPlayTrack={handlePlayTrack}
          onAddToPlaylist={handleAddToPlaylist}
          onToggleLike={handleToggleLike}
          onOpenArtist={handleOpenArtist}
          onOpenAuth={() => {
            settingsInitialTab = 'services';
            goTab('settings');
          }}
        />
      </div>

      <div class="tab-keep" class:tab-hidden={Boolean(activeArtist) || activeTab !== 'search'}>
        <SearchView
          initialQuery={quickSearch}
          {enabledSources}
          {authStatuses}
          {likedIds}
          localLibrary={[...likedTracks, ...recentTracks, ...localTracks]}
          onPlayTrack={handlePlayTrack}
          onToggleLike={handleToggleLike}
          onOpenArtist={handleOpenArtist}
          onOpenAuth={() => {
            settingsInitialTab = 'services';
            goTab('settings');
          }}
        />
      </div>

      {#if !activeArtist && activeTab === 'details'}
        <DetailsView
          track={player.current_track}
          queue={player.queue}
          shuffleOrder={player.shuffle_order ?? []}
          {waveActive}
          {waveTracks}
          {waveConfig}
          {enabledSources}
          mix={mixConfig}
          {activeDjPreset}
          {likedTracks}
          {recentTracks}
          {localPlaylists}
          {likedIds}
          {layoutStyle}
          tvWeatherLabel={atmosphere ? `${atmosphere.city} · ${atmosphere.summary}` : ''}
          isPlaying={player.is_playing}
          positionSec={player.position_sec}
          volume={player.volume}
          shuffle={player.shuffle}
          repeat={player.repeat}
          {effects}
          {authStatuses}
          onTogglePlay={handleToggle}
          onPrev={handlePrev}
          onNext={handleNext}
          onSeek={handleSeekVal}
          onVolumeChange={handleVolumeVal}
          onToggleShuffle={async () => applyStatePreservingPosition(await api.toggleShuffle())}
          onCycleRepeat={async () => applyStatePreservingPosition(await api.cycleRepeat())}
          onCyclePlaybackMode={cyclePlaybackMode}
          onPlayTrack={handlePlayTrack}
          onStartWave={handleStartWave}
          onResetWave={handleResetWave}
          onWaveConfigChange={updateWaveConfig}
          onToggleLike={handleToggleLike}
          onDislikeTrack={handleDislikeTrack}
          onOpenArtist={handleOpenArtist}
          onAddToPlaylist={handleAddToPlaylist}
          onToggleEq={() => saveEffects({ ...effects, eq_enabled: !effects.eq_enabled })}
          onOpenEqTab={() => goTab('equalizer')}
          onGoSettings={() => goTab('settings')}
          {audioProxyPort}
          onDuckMainAudio={(duck) => setMusicDucking(duck, 0.15)}
        />
      {:else if !activeArtist && activeTab === 'playlists'}
        <PlaylistsView
          playlists={playlists.filter((p) => !p.source || enabledSources.includes(p.source))}
          onCreatePlaylist={handleCreatePlaylist}
          onPlayTrack={handlePlayTrack}
          onRemoveTrack={handleRemoveFromPlaylist}
        />
      {:else if !activeArtist && activeTab === 'recap'}
        {#await import('./lib/components/RecapView.svelte') then { default: RecapView }}
          <RecapView
            currentTrackId={player.current_track?.id ?? null}
            {likedIds}
            onPlayTrack={handlePlayTrack}
            onToggleLike={handleToggleLike}
            onOpenArtist={handleOpenArtist}
          />
        {/await}
      {:else if !activeArtist && activeTab === 'room'}
        {#await import('./lib/components/RoomView.svelte') then { default: RoomView }}
          <RoomView
            room={jamRoom}
            {likedTracks}
            playlists={playlists}
            defaultName={sbProfile?.username || sources.find((s) => s.auth.connected)?.auth.account_name || ''}
            onCreateRoom={createJamRoom}
            onJoinRoom={joinJamRoom}
            onLeaveRoom={leaveJamRoom}
            onUpdateTaste={updateJamTaste}
            onPlayTrack={jamPlayTrack}
            onTogglePlay={jamTogglePlay}
            onSkip={jamSkip}
            onRebuildBlend={jamRebuildBlend}
            onSendChat={jamSendChat}
          />
        {/await}
      {:else if !activeArtist && activeTab === 'equalizer'}
        <div class="eq-wrap">
          {#await import('./lib/components/EqualizerPanel.svelte') then { default: EqualizerPanel }}
            <EqualizerPanel
              {effects}
              mix={mixConfig}
              shuffleMode={player.shuffle_mode ?? 'classic'}
              isPlaying={player.is_playing}
              {accentColor}
              onChange={saveEffects}
              onMixChange={updateMixConfig}
              onShuffleModeChange={async (mode) => applyStatePreservingPosition(await api.setShuffleMode(mode))}
            />
          {/await}
        </div>
      {:else if !activeArtist && activeTab === 'settings'}
        {#await import('./lib/components/SettingsView.svelte') then { default: SettingsView }}
          <SettingsView
            {sources}
            {enabledSources}
            onEnabledSourcesChange={updateEnabledSources}
            onSourcesChanged={(next) => {
              sources = next;
            }}
            onOpenGuide={() => (showOnboarding = true)}
            onOpenWhatsNew={() => (showWhatsNewModal = true)}
            atmosphereSummary={atmosphere ? `${atmosphere.city}: ${atmosphere.summary}` : ''}
            onDynamicThemeChange={(en, city) => refreshAtmosphere(en, city)}
            {activityConfig}
            onActivityConfigChange={updateActivityConfig}
            {playlists}
            {currentActivityLabel}
            {activeCssThemeId}
            {customThemeCss}
            onApplyCssTheme={applyCssTheme}
            radio={radioConfig}
            {radioStatus}
            onRadioChange={updateRadioConfig}
            onTestRadio={() => {
              const tr = player.current_track ?? likedTracks[0];
              if (tr) triggerRadioHost(tr, true);
            }}
            {ramSaverEnabled}
            onRamSaverChange={updateRamSaver}
            initialTab={settingsInitialTab}
            mix={mixConfig}
            onMixChange={updateMixConfig}
            {sbSession}
            {sbProfile}
            onAuthChange={async (s, p) => {
              sbSession = s;
              sbProfile = p;
              if (s) {
                const cloud = await sb.loadCloudSettings(s).catch(() => null);
                await applyCloudSettings(cloud);
              }
            }}
            onSyncCloudSettings={async () => {
              if (sbSession) {
                await sb.saveCloudSettings(sbSession, buildCloudSettingsPayload());
              }
            }}
          />
        {/await}
      {/if}
    </main>
  </div>

  <AccountModal
    open={showAccountModal}
    session={sbSession}
    profile={sbProfile}
    onClose={() => (showAccountModal = false)}
    onAuthChange={async (s, p) => {
      sbSession = s;
      sbProfile = p;
      if (s) {
        const cloud = await sb.loadCloudSettings(s).catch(() => null);
        await applyCloudSettings(cloud);
      }
    }}
    onSyncCloudSettings={async () => {
      if (sbSession) {
        await sb.saveCloudSettings(sbSession, buildCloudSettingsPayload());
      }
    }}
  />

  {#if showOnboarding}
    {#await import('./lib/components/OnboardingModal.svelte') then { default: OnboardingModal }}
      <OnboardingModal
        open={showOnboarding}
        onClose={() => (showOnboarding = false)}
        onGoSettings={() => goTab('settings')}
      />
    {/await}
  {/if}

  {#if showWhatsNewModal}
    {#await import('./lib/components/WhatsNewModal.svelte') then { default: WhatsNewModal }}
      <WhatsNewModal
        open={showWhatsNewModal}
        version={whatsNewVersion}
        notes={whatsNewNotes}
        date={whatsNewDate}
        onClose={() => (showWhatsNewModal = false)}
      />
    {/await}
  {/if}

  <RadixDialog
    open={showUpdateModal && Boolean(availableUpdateVersion)}
    onOpenChange={(v) => {
      if (!v && !installingOta) showUpdateModal = false;
    }}
    maxWidth="470px"
  >
    <div class="update-modal">
    <div class="update-head">
      <div class="update-head-left">
        <div class="update-icon-badge">
          <Sparkles size={18} />
        </div>
        <div>
          <div class="update-kicker">Доступно обновление Flowly</div>
          <h3>Версия v{availableUpdateVersion}</h3>
          {#if availableUpdateDate}
            <span class="update-date">
              {new Date(availableUpdateDate).toLocaleDateString('ru-RU', {
                day: 'numeric',
                month: 'long',
                year: 'numeric',
              })}
            </span>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="about-close"
        disabled={installingOta}
        onclick={() => (showUpdateModal = false)}
        aria-label="Закрыть"
      >
        <X size={16} />
      </button>
    </div>

    <div class="update-changelog-box">
      <div class="update-changelog-title">Список изменений</div>
      <div class="update-changelog-list">
        {#if availableUpdateNotes}
          {#each availableUpdateNotes.split(/\r?\n/).filter((l) => l.trim().length > 0) as line}
            {@const clean = line.trim().replace(/^[-*•]\s+/, '')}
            <div class="update-changelog-item">
              <span class="update-dot"></span>
              <span>{clean}</span>
            </div>
          {/each}
        {:else}
          <div class="update-changelog-item">
            <span class="update-dot"></span>
            <span>Улучшения стабильности, производительности и исправления ошибок.</span>
          </div>
        {/if}
      </div>
    </div>

    {#if installingOta}
      <div class="update-progress-wrap">
        <div class="update-progress-row">
          <span>Загрузка и установка обновления...</span>
          <span>{updateDownloadPct !== null ? `${updateDownloadPct}%` : '...'}</span>
        </div>
        <div class="update-progress-track">
          <div
            class="update-progress-fill"
            class:indeterminate={updateDownloadPct === null}
            style={updateDownloadPct !== null ? `width: ${updateDownloadPct}%` : ''}
          ></div>
        </div>
      </div>
    {/if}

    {#if updateInstallError}
      <div class="update-err">{updateInstallError}</div>
    {/if}

    <div class="update-actions">
      <button
        type="button"
        class="update-later-btn"
        disabled={installingOta}
        onclick={() => (showUpdateModal = false)}
      >
        Позже
      </button>
      <button
        type="button"
        class="update-now-btn"
        disabled={installingOta}
        onclick={installTopbarUpdate}
      >
        {#if installingOta}
          <RefreshCw size={14} class="spin" />
          <span>Установка...</span>
        {:else}
          <Download size={14} />
          <span>Обновить и перезапустить</span>
        {/if}
      </button>
    </div>
    </div>
  </RadixDialog>

  <!-- Парящий мини-плеер (скрыт на главном экране Details, чтобы не дублировать контролы) -->
  {#if activeTab !== 'details'}
    <footer class="player-bar">
      <div class="pb-top">
        <button type="button" class="np-left" onclick={() => (activeTab = 'details')}>
          {#if player.current_track}
            <img src={player.current_track.artwork} alt={player.current_track.title} />
            <div class="np-text">
              <div class="np-title"><MarqueeText text={player.current_track.title} paused={ramSaverEnabled} /></div>
              <div class="np-sub">
                <span>{player.current_track.artist}</span>
                <SourceBadge source={player.current_track.source} showLabel={false} />
              </div>
            </div>
          {:else}
            <div class="np-empty">Не выбрано</div>
          {/if}
        </button>

        <div class="np-core">
          {#if player.current_track}
            {@const isLiked = likedIds.has(player.current_track.id)}
            <button
              type="button"
              class="c-btn"
              class:liked-btn={isLiked}
              onclick={() => player.current_track && handleToggleLike(player.current_track)}
              aria-label="Нравится"
              title="Нравится"
            >
              <Heart size={15} fill={isLiked ? 'currentColor' : 'none'} />
            </button>
          {/if}
          <button
            type="button"
            class="c-btn"
            class:active={playbackModeInfo.active}
            onclick={cyclePlaybackMode}
            title="Режим: {playbackModeInfo.label}"
            aria-label="Режим воспроизведения"
          >
            {#if player.shuffle}
              <Shuffle size={14} />
            {:else if player.repeat === 'one'}
              <Repeat1 size={14} />
            {:else}
              <Repeat size={14} />
            {/if}
          </button>
          <button type="button" class="c-btn" onclick={handlePrev} aria-label="Previous">
            <SkipBack size={16} />
          </button>
          <button type="button" class="play-btn" onclick={handleToggle} aria-label="Play / Pause">
            {#if player.is_playing}
              <Pause size={15} fill="currentColor" />
            {:else}
              <Play size={15} fill="currentColor" />
            {/if}
          </button>
          <button type="button" class="c-btn" onclick={handleNext} aria-label="Next">
            <SkipForward size={16} />
          </button>
        </div>

        <div class="np-right">
          <button
            type="button"
            class="c-btn"
            onclick={() => goTab('equalizer')}
            title="Эквалайзер"
            aria-label="Эквалайзер"
          >
            <Sliders size={14} />
          </button>
          <div class="vol-wrap">
            {#if player.volume === 0}
              <VolumeX size={15} />
            {:else}
              <Volume2 size={15} />
            {/if}
            <RadixSlider
              value={player.volume}
              min={0}
              max={100}
              step={1}
              ariaLabel="Громкость"
              onValueChange={handleVolumeVal}
            />
            <span class="vol-pct">{player.volume}%</span>
          </div>
        </div>
      </div>

      <div class="seek-row">
        <span>{formatTime(player.position_sec)}</span>
        <RadixSlider
          value={player.position_sec}
          min={0}
          max={player.current_track?.duration ?? 100}
          step={1}
          disabled={!player.current_track}
          ariaLabel="Позиция трека"
          onValueChange={handleSeekVal}
        />
        <span>{formatTime(player.current_track?.duration ?? 0)}</span>
      </div>
    </footer>
  {/if}
</div>

<style>
  .shell {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-graphite);
    overflow: hidden;
  }

  .bg-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
    transition: opacity 0.8s ease, background 0.9s ease;
  }

  .bg-dynamic-tint {
    background:
      radial-gradient(ellipse 75% 55% at 20% 15%, var(--dyn-tint), transparent 70%),
      radial-gradient(ellipse 70% 55% at 85% 85%, var(--dyn-sub), transparent 70%);
    transition: background 1.2s ease;
  }

  .bg-ambient {
    inset: -60px;
    background:
      radial-gradient(circle 420px at 50% 42%, color-mix(in srgb, var(--ambient-1) 70%, transparent) 0%, color-mix(in srgb, var(--ambient-1) 28%, transparent) 46%, transparent 76%),
      radial-gradient(ellipse 560px 420px at 26% 28%, color-mix(in srgb, var(--ambient-2) 58%, transparent) 0%, transparent 72%),
      radial-gradient(ellipse 520px 460px at 74% 64%, color-mix(in srgb, var(--ambient-3) 54%, transparent) 0%, transparent 74%),
      radial-gradient(ellipse 480px 380px at 78% 24%, color-mix(in srgb, var(--ambient-4) 48%, transparent) 0%, transparent 70%),
      radial-gradient(ellipse 460px 400px at 22% 76%, color-mix(in srgb, var(--ambient-5) 46%, transparent) 0%, transparent 72%),
      radial-gradient(circle at 50% 48%, rgba(6, 6, 9, 0) 32%, rgba(6, 6, 9, 0.82) 100%);
    filter: blur(28px) saturate(1.85) brightness(1.18);
    opacity: 0.94;
  }

  .bg-ambient.playing {
    opacity: 1;
    animation: ambient-drift-a 11s ease-in-out infinite alternate;
  }

  .bg-ambient-orb {
    inset: -80px;
    background:
      radial-gradient(circle 340px at 64% 32%, color-mix(in srgb, var(--ambient-2) 50%, transparent) 0%, transparent 70%),
      radial-gradient(circle 320px at 34% 68%, color-mix(in srgb, var(--ambient-4) 46%, transparent) 0%, transparent 70%),
      radial-gradient(circle 280px at 76% 72%, color-mix(in srgb, var(--ambient-5) 42%, transparent) 0%, transparent 72%),
      radial-gradient(circle 300px at 28% 32%, color-mix(in srgb, var(--ambient-1) 44%, transparent) 0%, transparent 70%);
    filter: blur(44px) saturate(1.95) brightness(1.22);
    opacity: 0.6;
    mix-blend-mode: screen;
  }

  .bg-ambient-orb.playing {
    opacity: 0.85;
    animation: ambient-drift-b 15s ease-in-out infinite alternate;
  }

  @keyframes ambient-drift-a {
    0% {
      transform: translate3d(-1.5%, -1%, 0) scale(0.98) rotate(-2deg);
      filter: blur(28px) saturate(1.7) brightness(1.15);
    }
    50% {
      transform: translate3d(2%, 1.5%, 0) scale(1.08) rotate(2.5deg);
      filter: blur(34px) saturate(2.0) brightness(1.28);
    }
    100% {
      transform: translate3d(-0.5%, 2%, 0) scale(1.03) rotate(-1deg);
      filter: blur(30px) saturate(1.8) brightness(1.2);
    }
  }

  @keyframes ambient-drift-b {
    0% {
      transform: translate3d(3%, -2%, 0) scale(0.95) rotate(3deg);
    }
    50% {
      transform: translate3d(-3%, 2.5%, 0) scale(1.12) rotate(-4deg);
    }
    100% {
      transform: translate3d(1.5%, 3%, 0) scale(1.02) rotate(2deg);
    }
  }

  .bg-blur-cover {
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(56px) saturate(1.35);
    opacity: 0.34;
    transform: scale(1.08);
  }

  .bg-blur-scrim {
    background: radial-gradient(circle at 50% 35%, rgba(11, 11, 14, 0.45), rgba(11, 11, 14, 0.92) 80%);
  }

  .bg-aurora {
    background:
      radial-gradient(circle at 30% 20%, color-mix(in srgb, var(--ambient-1) 28%, transparent), transparent 52%),
      radial-gradient(circle at 75% 35%, color-mix(in srgb, var(--ambient-2) 22%, transparent), transparent 55%),
      radial-gradient(circle at 50% 85%, color-mix(in srgb, var(--ambient-3) 18%, transparent), transparent 55%);
    filter: blur(28px);
  }

  .bg-aurora.playing {
    animation: aurora-drift 14s ease-in-out infinite alternate;
  }

  @keyframes aurora-drift {
    0% {
      transform: translate3d(-2%, -1%, 0) scale(1);
    }
    100% {
      transform: translate3d(2%, 2%, 0) scale(1.06);
    }
  }

  .bg-spotlight {
    background:
      conic-gradient(from 165deg at 50% -10%, transparent 0deg, color-mix(in srgb, var(--ambient-1) 22%, transparent) 18deg, transparent 42deg),
      radial-gradient(ellipse 60% 36% at 50% 80%, color-mix(in srgb, var(--ambient-2) 10%, transparent), transparent 70%);
  }

  @media (prefers-reduced-motion: reduce) {
    .bg-ambient.playing,
    .bg-aurora.playing {
      animation: none;
    }
  }

  .bg-picker-wrap {
    position: relative;
  }

  .bg-btn.active {
    background: var(--bg-elevated);
    color: var(--accent);
  }

  .bg-popover {
    position: absolute;
    top: 32px;
    right: 0;
    width: 230px;
    padding: 8px;
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.65);
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 50;
  }

  .bg-pop-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 4px 8px;
  }

  .bg-pop-tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    padding: 3px;
    margin-bottom: 6px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.05);
  }

  .bg-pop-tabs button {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 700;
    padding: 5px 8px;
    border-radius: 7px;
    cursor: pointer;
  }

  .bg-pop-tabs button.active {
    background: var(--accent);
    color: #0b0b0e;
  }

  .bg-opt {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 7px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--text-primary);
  }

  .bg-opt:hover {
    background: var(--bg-elevated);
  }

  .bg-opt.active {
    background: color-mix(in srgb, var(--accent) 16%, var(--bg-elevated));
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .bg-opt-name {
    font-size: 12px;
    font-weight: 600;
  }

  .bg-opt-desc {
    font-size: 10px;
    color: var(--text-secondary);
  }

  .topbar {
    position: relative;
    z-index: 20;
    flex-shrink: 0;
    height: 46px;
    padding: 0 12px 0 16px;
    border-bottom: 1px solid var(--border-subtle);
    background: rgba(11, 11, 14, 0.88);
    display: grid;
    grid-template-columns: 200px 1fr 200px;
    align-items: center;
    gap: 12px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .brand-name {
    background: linear-gradient(135deg, #ffffff 30%, color-mix(in srgb, var(--accent) 65%, #ffffff));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .ota-pill {
    margin-left: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .top-search {
    justify-self: center;
    width: 100%;
    max-width: 380px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-radius: 10px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
  }

  .top-search input {
    width: 100%;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
  }

  .win-controls {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .win-btn {
    width: 28px;
    height: 24px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .win-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .win-btn.close:hover {
    background: #e11d48;
    color: white;
  }

  .body-frame {
    position: relative;
    z-index: 1;
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .icon-rail {
    width: 56px;
    flex-shrink: 0;
    border-right: 1px solid var(--border-subtle);
    background: rgba(19, 19, 24, 0.72);
    padding: 12px 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .rail-btn {
    position: relative;
    width: 38px;
    height: 38px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: background 0.14s ease, color 0.14s ease;
  }

  .rail-btn:hover {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .rail-btn.active {
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-elevated));
    color: var(--accent);
  }

  .rail-dot {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #10b981;
    box-shadow: 0 0 8px #10b981;
  }

  .rail-avatar {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    object-fit: cover;
  }

  .rail-initial {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 11px;
    font-weight: 800;
    display: grid;
    place-items: center;
  }

  .rail-spacer {
    flex: 1;
  }

  .main-area {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
  }

  .global-unauth-banner {
    margin: 14px 28px 0;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--accent, #6366f1) 12%, var(--bg-surface));
    border: 1px solid color-mix(in srgb, var(--accent, #6366f1) 34%, transparent);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .global-unauth-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }

  .global-unauth-copy strong {
    font-size: 13.5px;
    color: var(--text-primary);
  }

  .global-unauth-btn {
    flex-shrink: 0;
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent, #6366f1);
    color: #fff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .global-unauth-btn:hover {
    filter: brightness(1.08);
  }

  .tab-keep {
    display: contents;
  }

  .tab-keep.tab-hidden {
    display: none;
  }

  .eq-wrap {
    padding: 20px 28px;
  }

  .main-area.has-floating-bar {
    padding-bottom: 108px;
  }

  .player-bar {
    position: fixed;
    left: calc(50% + 28px);
    bottom: 16px;
    transform: translateX(-50%);
    z-index: 40;
    width: min(860px, calc(100vw - 104px));
    padding: 10px 16px 8px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.11);
    background: color-mix(in srgb, var(--bg-surface, #131318) 92%, #1a1b24);
    box-shadow:
      0 18px 48px rgba(0, 0, 0, 0.58),
      0 0 0 1px rgba(255, 255, 255, 0.04) inset;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pb-top {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 14px;
  }

  .np-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    cursor: pointer;
    border: none;
    background: transparent;
    text-align: left;
    color: inherit;
    padding: 0;
  }

  .np-left img {
    width: 40px;
    height: 40px;
    border-radius: 6px;
    object-fit: cover;
  }

  .np-text {
    min-width: 0;
  }

  .np-title {
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .np-sub {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .np-empty {
    font-size: 12px;
    color: var(--text-muted);
  }

  .np-core {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }

  .c-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    display: grid;
    place-items: center;
    padding: 4px;
    transition: color 130ms ease, transform 120ms ease;
  }

  .c-btn:hover,
  .c-btn.active {
    color: var(--accent);
  }

  .c-btn.liked-btn {
    color: var(--accent);
  }

  .play-btn {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: none;
    background: var(--text-primary);
    color: var(--bg-graphite);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: transform 120ms ease;
  }

  .play-btn:hover {
    transform: scale(1.06);
  }

  .seek-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 10px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .seek-row :global(.rx-slider-root) {
    flex: 1;
  }

  .np-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }

  .vol-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-secondary);
  }

  .vol-pct {
    font-size: 10.5px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    min-width: 28px;
    text-align: right;
  }

  .vol-wrap :global(.rx-slider-root) {
    width: 78px;
  }

  .about-close {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .about-close:hover {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-primary);
  }

  .update-modal {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .update-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .update-head-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .update-icon-badge {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, rgba(99, 102, 241, 0.28), rgba(168, 85, 247, 0.24));
    border: 1px solid rgba(129, 140, 248, 0.35);
    color: #c7d2fe;
    flex-shrink: 0;
  }

  .update-kicker {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent, #818cf8);
  }

  .update-head h3 {
    margin: 2px 0 0;
    font-size: 17px;
    color: var(--text-primary);
  }

  .update-date {
    display: inline-block;
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .update-changelog-box {
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 220px;
    overflow-y: auto;
  }

  .update-changelog-title {
    font-size: 11.5px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .update-changelog-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .update-changelog-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--text-secondary);
  }

  .update-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent, #818cf8);
    margin-top: 7px;
    flex-shrink: 0;
  }

  .update-progress-wrap {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .update-progress-row {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--text-secondary);
  }

  .update-progress-track {
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .update-progress-fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent, #6366f1), #a855f7);
    transition: width 0.2s ease;
  }

  .update-progress-fill.indeterminate {
    width: 45%;
    animation: update-slide 1.1s ease-in-out infinite alternate;
  }

  @keyframes update-slide {
    from {
      transform: translateX(-15%);
    }
    to {
      transform: translateX(135%);
    }
  }

  .update-err {
    padding: 9px 12px;
    border-radius: 9px;
    background: rgba(244, 63, 94, 0.14);
    border: 1px solid rgba(244, 63, 94, 0.35);
    color: #fda4af;
    font-size: 12px;
    line-height: 1.4;
  }

  .update-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 2px;
  }

  .update-later-btn,
  .update-now-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 9px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .update-later-btn {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--border-subtle);
    color: var(--text-secondary);
  }

  .update-later-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text-primary);
  }

  .update-now-btn {
    background: var(--accent, #6366f1);
    color: #fff;
  }

  .update-now-btn:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .update-later-btn:disabled,
  .update-now-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>

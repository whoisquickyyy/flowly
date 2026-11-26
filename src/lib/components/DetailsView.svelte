<script lang="ts">
  import {
    ExternalLink,
    Star,
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Shuffle,
    Repeat,
    Repeat1,
    Sliders,
    Music2,
    Volume2,
    VolumeX,
    Sparkles,
    ChevronDown,
    ChevronUp,
    Heart,
    ThumbsDown,
    RotateCcw,
    Plus,
    Share2,
    Check,
    Quote,
    GitBranch,
    MoreHorizontal,
    SlidersHorizontal,
    Radio,
    Maximize2,
    Minimize2,
  } from 'lucide-svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import SourceBadge from './SourceBadge.svelte';
  import LyricsPanel from './LyricsPanel.svelte';
  import ProgressBarMD3 from './ProgressBarMD3.svelte';
  import * as api from '../api';
  import type {
    AudioEffectsConfig,
    AuthStatus,
    LayoutStyle,
    MixConfig,
    MixPreset,
    Playlist,
    RepeatMode,
    SourceKind,
    Track,
    WaveConfig,
    WaveMood,
  } from '../types';

  interface Props {
    track: Track | null;
    queue: Track[];
    shuffleOrder?: string[];
    waveActive?: boolean;
    waveTracks?: Track[];
    waveConfig?: WaveConfig;
    enabledSources?: SourceKind[];
    mix?: MixConfig;
    activeDjPreset?: MixPreset;
    likedTracks?: Track[];
    recentTracks?: Track[];
    localPlaylists?: Playlist[];
    likedIds?: Set<string>;
    layoutStyle?: LayoutStyle;
    tvWeatherLabel?: string;
    isPlaying: boolean;
    positionSec: number;
    volume: number;
    shuffle: boolean;
    repeat: RepeatMode;
    effects: AudioEffectsConfig;
    authStatuses: AuthStatus[];
    onTogglePlay: () => void;
    onPrev: () => void;
    onNext: () => void;
    onSeek: (sec: number) => void;
    onVolumeChange: (vol: number) => void;
    onToggleShuffle?: () => void;
    onCycleRepeat?: () => void;
    onCyclePlaybackMode?: () => void;
    onPlayTrack: (track: Track, queue?: Track[]) => void;
    onStartWave: (sourceFilter: SourceKind | null) => Promise<void>;
    onResetWave?: () => Promise<void>;
    onWaveConfigChange?: (next: WaveConfig) => void;
    onToggleLike: (track: Track) => void;
    onDislikeTrack?: (track: Track) => void;
    onOpenArtist: (artist: string) => void;
    onAddToPlaylist: (playlistId: string, track: Track) => void;
    onToggleEq: () => void;
    onOpenEqTab: () => void;
    onGoSettings: () => void;
  }

  let {
    track,
    queue,
    shuffleOrder = [],
    waveActive = false,
    waveTracks = [],
    waveConfig = { sourceFilter: null, mood: 'mix' },
    enabledSources = ['yandex', 'spotify', 'soundcloud'],
    mix = { enabled: true, mode: 'auto', preset: 'dj-vocal', durationSec: 6, eqSweep: true },
    activeDjPreset = 'dj-vocal',
    likedTracks = [],
    recentTracks = [],
    localPlaylists = [],
    likedIds = new Set(),
    layoutStyle = 'wave-dock',
    tvWeatherLabel = '',
    isPlaying,
    positionSec,
    volume,
    shuffle,
    repeat,
    effects,
    authStatuses,
    onTogglePlay,
    onPrev,
    onNext,
    onSeek,
    onVolumeChange,
    onToggleShuffle = () => {},
    onCycleRepeat = () => {},
    onCyclePlaybackMode = () => {},
    onPlayTrack,
    onStartWave,
    onResetWave = async () => {},
    onWaveConfigChange = () => {},
    onToggleLike,
    onDislikeTrack = () => {},
    onOpenArtist,
    onAddToPlaylist,
    onToggleEq,
    onOpenEqTab,
    onGoSettings,
  }: Props = $props();

  let prevVolume = $state(80);
  let waveBusy = $state(false);
  let showWaveSettings = $state(false);
  let dockTab = $state<'liked' | 'recent' | 'queue'>('liked');
  let dockCollapsed = $state(false);
  let showVolPop = $state(false);
  let showMoreMenu = $state(false);
  let showLyrics = $state(false);
  let showSamples = $state(false);
  let samplesLoading = $state(false);
  let samplesTrackId = $state<string | null>(null);
  let samplesData = $state<import('../types').TrackSamplesResponse | null>(null);
  let playingSampleMbid = $state<string | null>(null);
  let copied = $state(false);
  let tvIdle = $state(false);
  let tvFullscreen = $state(false);
  let tvNow = $state(new Date());
  let tvIdleTimer: ReturnType<typeof setTimeout> | null = null;

  function bumpTvActivity() {
    tvIdle = false;
    if (tvIdleTimer) clearTimeout(tvIdleTimer);
    tvIdleTimer = setTimeout(() => {
      if (layoutStyle === 'tv-mode') tvIdle = true;
    }, 3500);
  }

  async function toggleTvFullscreen() {
    if (!api.isTauri()) return;
    try {
      const win = getCurrentWindow();
      const next = !tvFullscreen;
      await win.setFullscreen(next);
      tvFullscreen = next;
    } catch {
      // ignore
    }
  }

  $effect(() => {
    if (layoutStyle !== 'tv-mode') {
      tvIdle = false;
      return;
    }
    bumpTvActivity();
    const clock = setInterval(() => {
      tvNow = new Date();
    }, 15_000);
    return () => {
      clearInterval(clock);
      if (tvIdleTimer) clearTimeout(tvIdleTimer);
    };
  });

  $effect(() => {
    void track?.id;
    showSamples = false;
    showMoreMenu = false;
    showVolPop = false;
  });

  async function toggleSamplesPopover() {
    showSamples = !showSamples;
    showVolPop = false;
    showMoreMenu = false;
    if (!showSamples || !track) return;
    if (samplesTrackId === track.id && samplesData) return;

    samplesTrackId = track.id;
    samplesLoading = true;
    try {
      samplesData = await api.getTrackSamples(track.artist, track.title);
    } catch {
      samplesData = { samples: [], whosampledUrl: '' };
    } finally {
      samplesLoading = false;
    }
  }

  async function playSampleOriginal(sample: import('../types').TrackSample) {
    if (playingSampleMbid) return;
    playingSampleMbid = sample.mbid;
    try {
      const res = await api.searchGrouped(`${sample.artist} ${sample.title}`, null);
      const found = res.flatMap((g) => g.tracks)[0];
      if (found) {
        showSamples = false;
        onPlayTrack(found, [found]);
      }
    } finally {
      playingSampleMbid = null;
    }
  }

  const waveMoods: { id: WaveMood; label: string }[] = [
    { id: 'mix', label: 'Микс' },
    { id: 'energy', label: 'Энергия' },
    { id: 'chill', label: 'Чилл' },
    { id: 'discovery', label: 'Открытия' },
    { id: 'favorites', label: 'Любимое' },
  ];

  const djPresetLabels: Record<MixPreset, string> = {
    'dj-vocal': 'Beat + Vocal',
    'bass-swap': 'Bass Swap',
    rise: 'Rise',
    'echo-out': 'Echo Out',
    fade: 'Fade',
  };

  const playbackModeInfo = $derived.by(() => {
    if (shuffle) return { icon: 'shuffle-bold', label: 'Шафл', active: true };
    if (repeat === 'one') return { icon: 'repeat-one-bold', label: 'Повтор 1', active: true };
    if (repeat === 'all') return { icon: 'repeat-bold', label: 'Повтор', active: true };
    return { icon: 'repeat-linear', label: 'Порядок', active: false };
  });

  const isLiked = $derived(track ? likedIds.has(track.id) : false);

  function copyTrackLink() {
    if (!track) return;
    navigator.clipboard?.writeText(`${track.artist} — ${track.title}\n${track.externalUrl}`).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const currentAuth = $derived(
    track ? authStatuses.find((a) => a.source === track.source) ?? null : null
  );

  const leftWave = $derived(
    waveActive
      ? (waveTracks.length ? waveTracks : queue)
          .filter((t) => enabledSources.includes(t.source))
          .slice(0, 25)
      : []
  );
  const upNext = $derived.by(() => {
    const allowed = queue.filter((t) => enabledSources.includes(t.source));
    if (shuffle && !waveActive && shuffleOrder.length > 0) {
      const byId = new Map(allowed.map((t) => [t.id, t]));
      const ordered = shuffleOrder
        .map((id) => byId.get(id))
        .filter((t): t is Track => Boolean(t));
      if (ordered.length > 0) return ordered.slice(0, 15);
    }
    if (!track) return allowed.slice(0, 15);
    const idx = allowed.findIndex((t) => t.id === track.id);
    if (idx === -1) return allowed.slice(0, 15);
    return [...allowed.slice(idx + 1), ...allowed.slice(0, idx)].slice(0, 15);
  });

  const dockItems = $derived.by(() => {
    const allowedLiked = likedTracks.filter((t) => enabledSources.includes(t.source));
    const allowedRecent = recentTracks.filter((t) => enabledSources.includes(t.source));
    const allowedQueue = queue.filter((t) => enabledSources.includes(t.source));
    if (dockTab === 'liked') return allowedLiked.length ? allowedLiked.slice(0, 24) : allowedQueue;
    if (dockTab === 'recent') return allowedRecent.length ? allowedRecent.slice(0, 24) : allowedQueue;
    return allowedQueue;
  });

  function toggleMute() {
    if (volume > 0) {
      prevVolume = volume;
      onVolumeChange(0);
    } else {
      onVolumeChange(prevVolume || 80);
    }
  }

  async function launchWave() {
    waveBusy = true;
    try {
      await onStartWave(waveConfig.sourceFilter);
      showWaveSettings = false;
    } finally {
      waveBusy = false;
    }
  }

  function formatDuration(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

{#if layoutStyle === 'tv-mode'}
  <section
    class="tv-stage"
    class:tv-idle={tvIdle}
    role="presentation"
    onpointermove={bumpTvActivity}
    onclick={bumpTvActivity}
  >
    <button
      type="button"
      class="tv-fs-btn"
      onclick={toggleTvFullscreen}
      title={tvFullscreen ? 'Выйти из полного экрана' : 'Полный экран (TV)'}
      aria-label="Полный экран"
    >
      {#if tvFullscreen}
        <Minimize2 size={16} />
      {:else}
        <Maximize2 size={16} />
      {/if}
    </button>

    <div class="tv-left">
      {#if track}
        <div class="tv-cover-wrap">
          <img class="tv-cover" src={track.artwork} alt={track.title} />
        </div>
        <div class="tv-meta">
          <h1 class="tv-title">{track.title}</h1>
          <div class="tv-artist-row">
            <button type="button" class="artist-link" onclick={() => onOpenArtist(track.artist)}>
              {track.artist}
            </button>
            <SourceBadge source={track.source} showLabel={true} />
          </div>
          {#if track.releaseYear || track.genre}
            <div class="tv-sub">
              {track.album}
              {#if track.releaseYear} · {track.releaseYear}{/if}
              {#if track.genre} · {track.genre}{/if}
            </div>
          {/if}
        </div>

        <div class="tv-controls">
          <ProgressBarMD3 {positionSec} durationSec={track.duration} {isPlaying} {onSeek} />
          <div class="tv-transport">
            <button type="button" class="circle-act" class:liked={isLiked} onclick={() => onToggleLike(track)} aria-label="Лайк">
              <Heart size={15} fill={isLiked ? 'currentColor' : 'none'} />
            </button>
            <button type="button" class="t-btn" onclick={onPrev} aria-label="Назад">
              <SkipBack size={20} />
            </button>
            <button type="button" class="t-play" onclick={onTogglePlay} aria-label={isPlaying ? 'Пауза' : 'Плей'}>
              {#if isPlaying}
                <Pause size={22} fill="currentColor" />
              {:else}
                <Play size={22} fill="currentColor" />
              {/if}
            </button>
            <button type="button" class="t-btn" onclick={onNext} aria-label="Вперёд">
              <SkipForward size={20} />
            </button>
            <button type="button" class="circle-act" class:on={showLyrics} onclick={() => (showLyrics = !showLyrics)} title="Текст / Часы" aria-label="Текст">
              <Quote size={14} />
            </button>
          </div>
        </div>
      {/if}
    </div>

    <div class="tv-right">
      {#if showLyrics && track}
        <div class="tv-lyrics-wrap">
          <LyricsPanel {track} {positionSec} {isPlaying} {onSeek} />
        </div>
      {:else}
        <div class="tv-lounge">
          <div class="tv-clock">
            {tvNow.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}
          </div>
          <div class="tv-date">
            {tvNow.toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long' })}
          </div>
          {#if tvWeatherLabel}
            <div class="tv-weather">{tvWeatherLabel}</div>
          {/if}
          {#if upNext[0]}
            <button type="button" class="tv-upnext" onclick={() => onPlayTrack(upNext[0])}>
              <img src={upNext[0].artwork} alt={upNext[0].title} />
              <div>
                <span class="tv-un-lbl">Далее в очереди</span>
                <div class="tv-un-title">{upNext[0].title}</div>
                <div class="tv-un-art">{upNext[0].artist}</div>
              </div>
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </section>
{:else}
<div class="details-root">
  <section class="sc-stage">
    <!-- Левая колонка: Моя волна -->
    <aside class="side-panel">
        <div class="wave-head-row">
          <div class="panel-head">Волна</div>
          <div class="wave-head-acts">
            {#if waveActive}
              <button
                type="button"
                class="wave-gear"
                onclick={onResetWave}
                title="Сбросить волну и включить случайный трек из библиотеки"
                aria-label="Сбросить волну"
              >
                <RotateCcw size={12} />
              </button>
              <button
                type="button"
                class="wave-gear"
                class:on={showWaveSettings}
                onclick={() => (showWaveSettings = !showWaveSettings)}
                title="Настройки волны"
                aria-label="Настройки волны"
              >
                <SlidersHorizontal size={12} />
              </button>
              <button type="button" class="wave-go" disabled={waveBusy} onclick={launchWave}>
                <Sparkles size={11} />
                <span>{waveBusy ? 'Подбор...' : 'Обновить'}</span>
              </button>
            {/if}
          </div>
        </div>

        {#if !waveActive}
          <div class="wave-stub">
            <div class="wave-stub-icon">
              <Radio size={22} />
            </div>
            <div class="wave-stub-title">Моя волна</div>
            <p class="wave-stub-desc">
              Бесконечный поток под ваш вкус. Выберите источники и характер волны:
            </p>

            {#if enabledSources.length > 1}
              <div class="wave-sources">
                <button
                  type="button"
                  class:active={waveConfig.sourceFilter === null}
                  onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: null })}
                >
                  Все
                </button>
                {#if enabledSources.includes('yandex')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'yandex'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'yandex' })}
                  >
                    ЯМ
                  </button>
                {/if}
                {#if enabledSources.includes('spotify')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'spotify'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'spotify' })}
                  >
                    Spotify
                  </button>
                {/if}
                {#if enabledSources.includes('soundcloud')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'soundcloud'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'soundcloud' })}
                  >
                    SC
                  </button>
                {/if}
              </div>
            {/if}

            <div class="wave-moods">
              {#each waveMoods as m (m.id)}
                <button
                  type="button"
                  class:active={waveConfig.mood === m.id}
                  onclick={() => onWaveConfigChange({ ...waveConfig, mood: m.id })}
                >
                  {m.label}
                </button>
              {/each}
            </div>

            <button type="button" class="wave-start-btn" disabled={waveBusy} onclick={launchWave}>
              <Sparkles size={14} />
              <span>{waveBusy ? 'Подбираем треки...' : 'Включить волну'}</span>
            </button>
          </div>
        {:else}
          <div class="wave-stub active-wave">
            <div class="wave-stub-icon" class:spinning={isPlaying}>
              <Radio size={22} />
            </div>
            <div class="wave-stub-title">Волна активна</div>
            <p class="wave-stub-desc">
              Бесконечный поток подбирается автоматически без повторов.
            </p>

            {#if enabledSources.length > 1}
              <div class="wave-sources">
                <button
                  type="button"
                  class:active={waveConfig.sourceFilter === null}
                  onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: null })}
                >
                  Все
                </button>
                {#if enabledSources.includes('yandex')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'yandex'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'yandex' })}
                  >
                    ЯМ
                  </button>
                {/if}
                {#if enabledSources.includes('spotify')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'spotify'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'spotify' })}
                  >
                    Spotify
                  </button>
                {/if}
                {#if enabledSources.includes('soundcloud')}
                  <button
                    type="button"
                    class:active={waveConfig.sourceFilter === 'soundcloud'}
                    onclick={() => onWaveConfigChange({ ...waveConfig, sourceFilter: 'soundcloud' })}
                  >
                    SC
                  </button>
                {/if}
              </div>
            {/if}

            <div class="wave-moods">
              {#each waveMoods as m (m.id)}
                <button
                  type="button"
                  class:active={waveConfig.mood === m.id}
                  onclick={() => onWaveConfigChange({ ...waveConfig, mood: m.id })}
                >
                  {m.label}
                </button>
              {/each}
            </div>

            <button type="button" class="wave-start-btn" disabled={waveBusy} onclick={launchWave}>
              <Sparkles size={14} />
              <span>{waveBusy ? 'Подбираем...' : 'Применить и обновить'}</span>
            </button>
          </div>
        {/if}
      </aside>

    <!-- Центральная сцена -->
    <div class="now-center">
      {#if track}
        <div class="np-caption">Сейчас играет</div>
        <h1 class="np-heading">{track.title}</h1>

        <div class="artwork-wrap" class:playing={isPlaying} class:compact-art={layoutStyle === 'wave-dock' && !dockCollapsed} class:lyrics-art={showLyrics}>
          <div class="artwork-glow"></div>
          {#key track.id}
            <img class="artwork-img" src={track.artwork} alt={track.title} />
          {/key}
        </div>

        <div class="artist-row">
          <button
            type="button"
            class="artist-link"
            onclick={() => onOpenArtist(track.artist)}
            title="Открыть треки артиста"
          >
            {track.artist}
          </button>
          <SourceBadge source={track.source} showLabel={true} />
        </div>

        <div class="meta-line">
          <span>{track.album}</span>
          {#if track.releaseYear}
            <span>·</span>
            <span>{track.releaseYear}</span>
          {/if}
          {#if track.genre}
            <span>·</span>
            <span>{track.genre}</span>
          {/if}
          {#if track.rating !== null}
            <span>·</span>
            <span class="rating"><Star size={11} fill="currentColor" /> {track.rating.toFixed(1)}</span>
          {/if}
        </div>

        <!-- Прогресс трека (MD3 Expressive) + компактные действия: Лайк | Прогресс | Громкость | Текст | Три точки -->
        <div class="wave-pill-row">
          <button
            type="button"
            class="circle-act like-btn"
            class:liked={isLiked}
            onclick={() => onToggleLike(track)}
            title={isLiked ? 'Убрать из любимых' : 'Добавить в любимые'}
            aria-label="Лайк"
          >
            <Heart size={14} fill={isLiked ? 'currentColor' : 'none'} />
          </button>

          <button
            type="button"
            class="circle-act dislike-btn"
            onclick={() => onDislikeTrack(track)}
            title="Не нравится (пропустить и не включать)"
            aria-label="Дизлайк"
          >
            <ThumbsDown size={13} />
          </button>

          <ProgressBarMD3
            {positionSec}
            durationSec={track.duration}
            {isPlaying}
            {onSeek}
          />

          {#if layoutStyle === 'wave-dock'}
          <div class="vol-pop-wrap">
            <button
              type="button"
              class="circle-act"
              class:on={showVolPop}
              onclick={() => {
                showVolPop = !showVolPop;
                showMoreMenu = false;
              }}
              title="Громкость: {volume}%"
              aria-label="Громкость"
            >
              {#if volume === 0}
                <VolumeX size={14} />
              {:else}
                <Volume2 size={14} />
              {/if}
            </button>

            {#if showVolPop}
              <div class="vol-popover">
                <button type="button" class="vol-mute" onclick={toggleMute} aria-label="Mute">
                  {#if volume === 0}
                    <VolumeX size={14} />
                  {:else}
                    <Volume2 size={14} />
                  {/if}
                </button>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={volume}
                  aria-label="Громкость"
                  oninput={(e) => onVolumeChange(Number((e.target as HTMLInputElement).value))}
                />
                <span class="vol-pct">{volume}%</span>
              </div>
            {/if}
          </div>
          {/if}

          <button
            type="button"
            class="circle-act"
            class:on={showLyrics}
            onclick={() => (showLyrics = !showLyrics)}
            title={showLyrics ? 'Показать обложку' : 'Текст песни (Apple Music стиль)'}
            aria-label="Текст песни"
          >
            <Quote size={14} />
          </button>

          <div class="vol-pop-wrap">
            <button
              type="button"
              class="circle-act"
              class:on={showMoreMenu || showSamples}
              onclick={() => {
                if (showSamples) {
                  showSamples = false;
                } else {
                  showMoreMenu = !showMoreMenu;
                }
                showVolPop = false;
              }}
              title="Ещё"
              aria-label="Ещё действия"
            >
              <MoreHorizontal size={15} />
            </button>

            {#if showSamples}
              <div class="samples-popover">
                <div class="pl-pop-title">Сэмплы в треке</div>
                {#if samplesLoading}
                  <div class="samples-msg">Поиск сэмплов...</div>
                {:else if samplesData && samplesData.samples.length > 0}
                  <div class="samples-list">
                    {#each samplesData.samples as smp (smp.mbid)}
                      <div class="sample-item">
                        <div class="sample-meta">
                          <span class="sample-rel">{smp.relation}</span>
                          <div class="sample-title">{smp.title}</div>
                          <div class="sample-artist">{smp.artist}</div>
                        </div>
                        <button
                          type="button"
                          class="sample-play"
                          disabled={playingSampleMbid === smp.mbid}
                          onclick={() => playSampleOriginal(smp)}
                          title="Включить оригинальный трек"
                        >
                          <Play size={12} fill="currentColor" />
                          <span>{playingSampleMbid === smp.mbid ? '...' : 'Оригинал'}</span>
                        </button>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <div class="samples-msg">Сэмплы в базе MusicBrainz не найдены.</div>
                {/if}
                {#if samplesData?.whosampledUrl}
                  <button
                    type="button"
                    class="pl-pop-item"
                    onclick={() => samplesData?.whosampledUrl && api.openExternalUrl(samplesData.whosampledUrl)}
                  >
                    <ExternalLink size={13} />
                    <span>Открыть на WhoSampled</span>
                  </button>
                {/if}
              </div>
            {/if}

            {#if showMoreMenu}
              <div class="more-popover">
                <button
                  type="button"
                  class="pl-pop-item"
                  onclick={toggleSamplesPopover}
                >
                  <GitBranch size={13} />
                  <span>Сэмплы (WhoSampled)</span>
                </button>
                <button
                  type="button"
                  class="pl-pop-item"
                  onclick={() => {
                    copyTrackLink();
                    showMoreMenu = false;
                  }}
                >
                  {#if copied}
                    <Check size={13} />
                    <span>Ссылка скопирована</span>
                  {:else}
                    <Share2 size={13} />
                    <span>Поделиться ссылкой</span>
                  {/if}
                </button>
                <button
                  type="button"
                  class="pl-pop-item"
                  onclick={() => {
                    api.openExternalUrl(track.externalUrl);
                    showMoreMenu = false;
                  }}
                >
                  <ExternalLink size={13} />
                  <span>Открыть в источнике</span>
                </button>
                {#if localPlaylists.length > 0}
                  <div class="more-sep"></div>
                  <div class="pl-pop-title">В плейлист</div>
                  {#each localPlaylists as pl (pl.id)}
                    <button
                      type="button"
                      class="pl-pop-item"
                      onclick={() => {
                        onAddToPlaylist(pl.id, track);
                        showMoreMenu = false;
                      }}
                    >
                      <Plus size={13} />
                      <span>{pl.title}</span>
                    </button>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        </div>

        <!-- Центральное управление: идеально отцентрировано по обложке -->
        <div class="transport">
          <div class="transport-side left"></div>
          <div class="transport-core">
            <button type="button" class="t-btn" onclick={onPrev} aria-label="Предыдущий трек">
              <SkipBack size={18} />
            </button>
            <button
              type="button"
              class="t-play"
              onclick={onTogglePlay}
              aria-label={isPlaying ? 'Пауза' : 'Воспроизвести'}
            >
              {#if isPlaying}
                <Pause size={20} fill="currentColor" />
              {:else}
                <Play size={20} fill="currentColor" />
              {/if}
            </button>
            <button type="button" class="t-btn" onclick={onNext} aria-label="Следующий трек">
              <SkipForward size={18} />
            </button>
          </div>
          <div class="transport-side right">
            <button
              type="button"
              class="t-btn"
              class:active={playbackModeInfo.active}
              onclick={onCyclePlaybackMode}
              title="Режим воспроизведения: {playbackModeInfo.label} (нажмите для смены)"
              aria-label="Режим воспроизведения"
            >
              {#if shuffle}
                <Shuffle size={15} />
              {:else if repeat === 'one'}
                <Repeat1 size={15} />
              {:else}
                <Repeat size={15} />
              {/if}
            </button>
          </div>
        </div>

        {#if layoutStyle !== 'wave-dock'}
          <div class="stage-volume">
            <button type="button" class="vol-mute" onclick={toggleMute} aria-label="Mute">
              {#if volume === 0}
                <VolumeX size={14} />
              {:else}
                <Volume2 size={14} />
              {/if}
            </button>
            <input
              type="range"
              min="0"
              max="100"
              value={volume}
              aria-label="Громкость"
              oninput={(e) => onVolumeChange(Number((e.target as HTMLInputElement).value))}
            />
            <span class="vol-pct">{volume}%</span>
          </div>
        {/if}

        {#if showLyrics}
          <div class="inline-lyrics-slot">
            <LyricsPanel
              {track}
              {positionSec}
              {isPlaying}
              {onSeek}
            />
          </div>
        {/if}

        {#if currentAuth && !currentAuth.connected}
          <button type="button" class="connect-hint" onclick={onGoSettings}>
            Подключить {currentAuth.source} для полного трека
          </button>
        {/if}
      {:else}
        <div class="empty-stage">
          <Music2 size={40} />
          <h2>Нет активного трека</h2>
          <p>Запустите «Мою волну» слева или выберите трек в Библиотеке / Поиске.</p>
        </div>
      {/if}
    </div>

    <!-- Правая колонка: Далее + Настройки звука -->
    <aside class="side-panel" class:analog-right={layoutStyle === 'modern-analog'}>
      <div class="panel-head">{waveActive ? 'Далее в волне' : 'Далее'}</div>
      <div class="track-stack">
        {#each (layoutStyle === 'modern-analog' ? upNext.slice(0, 1) : upNext) as item (item.id)}
          <button type="button" class="stack-item" onclick={() => onPlayTrack(item)}>
            <img src={item.artwork} alt={item.title} />
            <div class="s-meta">
              <div class="s-title">{item.title}</div>
              <div class="s-artist">{item.artist}</div>
            </div>
            <span class="s-dur">{formatDuration(item.duration)}</span>
          </button>
        {/each}
      </div>

      <div class="dsp-box">
        <div class="dsp-head">
          <span>Настройки звука</span>
        </div>
        <div class="dsp-pills">
          <button type="button" class="dsp-pill" class:on={effects.eq_enabled} onclick={onToggleEq}>
            <Sliders size={12} />
            <span>EQ: {effects.eq_enabled ? effects.preset : 'Выкл'}</span>
          </button>
          <button type="button" class="dsp-pill" class:on={effects.normalize_volume} onclick={onOpenEqTab}>
            <span>Нормализация</span>
          </button>
          <button type="button" class="dsp-pill" class:on={effects.spatial_audio} onclick={onOpenEqTab}>
            <span>3D-сцена</span>
          </button>
          <button type="button" class="dsp-pill" class:on={mix.enabled} onclick={onOpenEqTab}>
            <span>{mix.enabled ? (mix.mode === 'auto' ? `DJ: ${djPresetLabels[activeDjPreset]}` : `Mix: ${djPresetLabels[mix.preset]}`) : 'Mix Выкл'}</span>
          </button>
        </div>
      </div>
    </aside>
  </section>

  <!-- Нижняя полка Wave Dock -->
  {#if layoutStyle === 'wave-dock'}
    <section class="bottom-dock" class:collapsed={dockCollapsed}>
      <div class="dock-tabs">
        <div class="dock-pills">
          <button type="button" class:active={dockTab === 'liked'} onclick={() => (dockTab = 'liked')}>
            Любимые треки
          </button>
          <button type="button" class:active={dockTab === 'recent'} onclick={() => (dockTab = 'recent')}>
            Недавние
          </button>
          <button type="button" class:active={dockTab === 'queue'} onclick={() => (dockTab = 'queue')}>
            Очередь волны
          </button>
        </div>
        <button
          type="button"
          class="dock-collapse"
          onclick={() => (dockCollapsed = !dockCollapsed)}
          aria-label={dockCollapsed ? 'Развернуть полку' : 'Свернуть полку'}
        >
          {#if dockCollapsed}
            <ChevronUp size={15} />
          {:else}
            <ChevronDown size={15} />
          {/if}
        </button>
      </div>

      {#if !dockCollapsed}
        <div
          class="dock-strip"
          onwheel={(e) => {
            if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
              e.preventDefault();
              e.currentTarget.scrollLeft += e.deltaY;
            }
          }}
        >
          {#each dockItems as item (item.id)}
            {@const active = track?.id === item.id}
            <button
              type="button"
              class="dock-card"
              class:active
              onclick={() => {
                const fullPool =
                  dockTab === 'liked'
                    ? likedTracks.filter((t) => enabledSources.includes(t.source))
                    : dockTab === 'recent'
                      ? recentTracks.filter((t) => enabledSources.includes(t.source))
                      : queue.filter((t) => enabledSources.includes(t.source));
                onPlayTrack(item, fullPool.length ? fullPool : dockItems);
              }}
              title="{item.title} — {item.artist}"
            >
              <img src={item.artwork} alt={item.title} loading="lazy" />
              <span class="dock-heart">
                <Heart size={10} fill="currentColor" />
              </span>
              <span class="dock-src">
                <SourceBadge source={item.source} showLabel={false} />
              </span>
              <div class="dock-overlay">
                <div class="dock-t">{item.title}</div>
                <div class="dock-a">{item.artist}</div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>
{/if}

<style>
  .details-root {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .sc-stage {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: grid;
    grid-template-columns: 248px 1fr 258px;
    gap: 18px;
    padding: 14px 20px 10px;
    align-items: stretch;
  }

  .inline-lyrics-slot {
    width: 100%;
    max-width: 600px;
    height: 196px;
    margin-top: 14px;
    animation: pop-in 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .side-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }

  .panel-head {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
    padding: 0 4px;
  }

  .wave-head-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .wave-go {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 999px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    transition: transform 0.12s ease, opacity 0.15s ease;
  }

  .wave-go:hover {
    transform: scale(1.03);
  }

  .wave-go:disabled {
    opacity: 0.6;
    cursor: wait;
  }

  .wave-sources {
    display: flex;
    gap: 4px;
  }

  .wave-sources button {
    flex: 1;
    padding: 4px 6px;
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.02);
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
  }

  .wave-sources button.active {
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-elevated));
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--text-primary);
  }

  .track-stack {
    display: flex;
    flex-direction: column;
    gap: 5px;
    overflow-y: auto;
  }

  .stack-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 9px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.04);
    background: rgba(255, 255, 255, 0.025);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.14s ease, border-color 0.14s ease, transform 0.1s ease;
  }

  .stack-item:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .stack-item img {
    width: 36px;
    height: 36px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .s-meta {
    flex: 1;
    min-width: 0;
  }

  .s-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-artist {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-dur {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .dsp-box {
    margin-top: auto;
    padding: 12px;
    border-radius: 12px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .side-panel.analog-right .track-stack {
    flex: 0 0 auto;
    overflow: visible;
  }

  .side-panel.analog-right .dsp-box {
    margin-top: 6px;
    flex: 0 0 auto;
    padding: 13px 14px;
    gap: 10px;
  }

  .side-panel.analog-right .dsp-head {
    font-size: 12px;
  }

  .side-panel.analog-right .dsp-pills {
    gap: 8px;
  }

  .side-panel.analog-right .dsp-pill {
    padding: 7px 11px;
    font-size: 11.5px;
  }

  .dsp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .dsp-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .dsp-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 9px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
  }

  .dsp-pill.on {
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .now-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    min-width: 0;
    min-height: 0;
  }

  .np-caption {
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.72);
    text-shadow: 0 1px 10px rgba(0, 0, 0, 0.65);
  }

  .np-heading {
    font-size: clamp(24px, 2.8vw, 34px);
    font-weight: 700;
    letter-spacing: -0.025em;
    text-align: center;
    max-width: 520px;
    line-height: 1.12;
    margin-bottom: 6px;
    text-shadow: 0 2px 16px rgba(0, 0, 0, 0.65);
  }

  .artwork-wrap {
    position: relative;
    width: 260px;
    height: 260px;
    display: grid;
    place-items: center;
    transition: width 0.2s ease, height 0.2s ease;
  }

  .artwork-wrap.compact-art {
    width: 224px;
    height: 224px;
  }

  .artwork-wrap.lyrics-art {
    width: 196px;
    height: 196px;
  }

  .artwork-glow {
    position: absolute;
    inset: -14px;
    border-radius: 24px;
    background: radial-gradient(circle, color-mix(in srgb, var(--accent) 34%, transparent), transparent 70%);
    filter: blur(22px);
    opacity: 0.65;
    pointer-events: none;
    transition: opacity 0.4s ease, transform 0.4s ease;
  }

  .artwork-wrap.playing .artwork-glow {
    opacity: 1;
    animation: art-glow-pulse 2.8s ease-in-out infinite alternate;
  }

  @keyframes art-glow-pulse {
    from {
      transform: scale(0.97);
      opacity: 0.78;
    }
    to {
      transform: scale(1.07);
      opacity: 1;
    }
  }

  .artwork-img {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    border-radius: 16px;
    object-fit: cover;
    border: 1px solid var(--border-strong);
    box-shadow: 0 22px 48px rgba(0, 0, 0, 0.65);
    animation: cover-enter 0.28s cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes cover-enter {
    from {
      opacity: 0.65;
      transform: scale(0.96);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .artist-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }

  .artist-link {
    border: none;
    background: transparent;
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.88);
    text-shadow: 0 1px 10px rgba(0, 0, 0, 0.6);
    cursor: pointer;
    padding: 0;
    transition: color 0.14s ease;
  }

  .artist-link:hover {
    color: var(--text-primary);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .like-btn.liked {
    color: #ff3b5c;
    border-color: rgba(255, 59, 92, 0.45);
    background: rgba(255, 59, 92, 0.12);
    animation: heart-pop 0.25s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .dislike-btn:hover {
    color: #fb7185;
    border-color: rgba(251, 113, 133, 0.45);
    background: rgba(251, 113, 133, 0.12);
  }

  @keyframes heart-pop {
    0% { transform: scale(1); }
    50% { transform: scale(1.22); }
    100% { transform: scale(1); }
  }

  .more-popover {
    position: absolute;
    bottom: 42px;
    right: 0;
    min-width: 196px;
    padding: 6px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.72);
    z-index: 35;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .more-sep {
    height: 1px;
    background: var(--border-subtle);
    margin: 4px 2px;
  }

  .samples-popover {
    position: absolute;
    bottom: 38px;
    right: 0;
    width: 280px;
    padding: 10px;
    border-radius: 14px;
    background: rgba(16, 16, 22, 0.96);
    border: 1px solid var(--border-subtle);
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(18px);
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .samples-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
  }

  .sample-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
  }

  .sample-meta {
    flex: 1;
    min-width: 0;
  }

  .sample-rel {
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent);
  }

  .sample-title {
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sample-artist {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sample-play {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 9px;
    border-radius: 999px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    flex-shrink: 0;
  }

  .samples-msg {
    font-size: 11.5px;
    color: var(--text-secondary);
    padding: 6px 4px;
  }

  .pl-pop-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-muted);
    padding: 3px 8px;
  }

  .pl-pop-item {
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 12px;
    text-align: left;
    padding: 7px 9px;
    border-radius: 8px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pl-pop-item:hover {
    background: var(--bg-elevated);
  }

  .wave-head-acts {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .wave-gear {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: all 0.14s ease;
  }

  .wave-gear:hover,
  .wave-gear.on {
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .wave-stub {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 10px;
    padding: 18px 12px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px dashed rgba(255, 255, 255, 0.1);
  }

  .wave-stub-icon {
    width: 46px;
    height: 46px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    color: var(--accent);
    display: grid;
    place-items: center;
    animation: art-glow-pulse 2.6s ease-in-out infinite alternate;
  }

  .wave-stub-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .wave-stub-desc {
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-secondary);
    max-width: 200px;
  }

  .wave-moods {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 5px;
    width: 100%;
  }

  .wave-moods button {
    padding: 4px 8px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.14s ease;
  }

  .wave-moods button.active {
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .wave-start-btn {
    margin-top: 4px;
    width: 100%;
    padding: 9px 12px;
    border-radius: 999px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 12px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    cursor: pointer;
    transition: transform 0.14s ease, filter 0.14s ease;
  }

  .wave-start-btn:hover {
    transform: translateY(-1px);
    filter: brightness(1.08);
  }

  .wave-start-btn:disabled {
    opacity: 0.65;
    cursor: wait;
  }

  .meta-line {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.72);
    text-shadow: 0 1px 10px rgba(0, 0, 0, 0.65);
    margin-top: -4px;
  }

  .rating {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--accent);
  }

  /* Pill Waveform */
  .wave-pill-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 500px;
  }

  .circle-act {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    flex-shrink: 0;
    transition: color 0.14s ease, background 0.14s ease, border-color 0.14s ease;
  }

  .circle-act:hover,
  .circle-act.on {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.09);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .vol-pop-wrap {
    position: relative;
  }

  .vol-popover {
    position: absolute;
    bottom: 42px;
    right: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.65);
    z-index: 35;
    width: 232px;
  }

  .vol-popover input[type='range'] {
    flex: 1;
    min-width: 110px;
    height: 4px;
    cursor: pointer;
  }

  .transport {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    width: 100%;
    max-width: 440px;
    gap: 12px;
  }

  .transport-core {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }

  .transport-side {
    display: flex;
    align-items: center;
  }

  .transport-side.right {
    justify-content: flex-start;
  }

  .t-btn {
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: color 0.14s ease, background-color 0.14s ease, transform 0.12s ease;
  }

  .t-btn:active,
  .circle-act:active,
  .t-play:active {
    transform: scale(0.92);
  }

  .t-btn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.08);
  }

  .t-btn.active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: 0 0 12px color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .more-popover,
  .vol-popover {
    animation: pop-in 0.16s cubic-bezier(0.22, 1, 0.36, 1);
    transform-origin: bottom right;
  }

  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .t-play {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-primary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: transform 0.14s ease, background 0.14s ease;
  }

  .t-play:hover {
    transform: scale(1.06);
    background: var(--accent);
    color: #0b0b0e;
    border-color: transparent;
  }

  .stage-volume {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 232px;
    padding: 6px 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-subtle);
  }

  .vol-mute {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 2px;
    flex-shrink: 0;
  }

  .stage-volume input[type='range'] {
    flex: 1;
    min-width: 0;
    height: 4px;
    cursor: pointer;
  }

  .vol-pct {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    min-width: 34px;
    flex-shrink: 0;
    text-align: right;
  }

  .connect-hint {
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    font-size: 11px;
    padding: 4px 12px;
    border-radius: 999px;
    cursor: pointer;
  }

  /* Нижняя полка Wave Dock */
  .bottom-dock {
    flex-shrink: 0;
    padding: 8px 20px 12px;
    border-top: 1px solid var(--border-subtle);
    background: rgba(11, 11, 14, 0.55);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dock-tabs {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .dock-pills {
    display: flex;
    gap: 6px;
  }

  .dock-pills button {
    padding: 5px 12px;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .dock-pills button.active {
    background: var(--text-primary);
    color: var(--bg-graphite);
  }

  .dock-collapse {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: none;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .dock-strip {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .dock-card {
    position: relative;
    width: 92px;
    height: 92px;
    flex-shrink: 0;
    border-radius: 10px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    padding: 0;
    background: var(--bg-surface);
    cursor: pointer;
    transition: transform 0.14s ease, border-color 0.14s ease;
  }

  .dock-card:hover {
    transform: translateY(-2px);
    border-color: var(--border-strong);
  }

  .dock-card.active {
    border: 2px solid var(--accent);
  }

  .dock-card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .dock-heart {
    position: absolute;
    top: 5px;
    left: 5px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgba(11, 11, 14, 0.72);
    color: var(--text-primary);
    display: grid;
    place-items: center;
  }

  .dock-src {
    position: absolute;
    top: 5px;
    right: 5px;
  }

  .dock-overlay {
    position: absolute;
    inset: auto 0 0 0;
    padding: 14px 6px 5px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.85));
    text-align: left;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .dock-card:hover .dock-overlay,
  .dock-card.active .dock-overlay {
    opacity: 1;
  }

  .dock-t {
    font-size: 10px;
    font-weight: 600;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dock-a {
    font-size: 9px;
    color: rgba(255, 255, 255, 0.7);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .empty-stage {
    text-align: center;
    color: var(--text-secondary);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  @media (prefers-reduced-motion: reduce) {
    .artwork-img,
    .dock-card,
    .t-play {
      animation: none;
      transition: none;
    }
  }

  @media (max-height: 780px) {
    .artwork-wrap.compact-art {
      width: 184px;
      height: 184px;
    }
    .artwork-wrap.lyrics-art {
      width: 152px;
      height: 152px;
    }
    .inline-lyrics-slot {
      height: 152px;
    }
  }

  @media (max-width: 980px) {
    .sc-stage {
      grid-template-columns: 1fr;
    }
    .side-panel {
      display: none;
    }
  }

  /* ── TV Mode ────────────────────────────────────────────── */
  .tv-stage {
    position: relative;
    height: 100%;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(340px, 1.05fr) minmax(320px, 0.95fr);
    align-items: center;
    gap: clamp(28px, 4vw, 64px);
    padding: clamp(24px, 4vh, 48px) clamp(28px, 5vw, 72px);
    overflow: hidden;
  }

  .tv-fs-btn {
    position: absolute;
    top: 18px;
    right: 22px;
    z-index: 5;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(15, 20, 32, 0.52);
    backdrop-filter: blur(12px);
    color: var(--text-primary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: opacity 0.35s ease, transform 0.2s ease, background 0.2s ease;
  }

  .tv-fs-btn:hover {
    background: rgba(255, 255, 255, 0.16);
    transform: scale(1.06);
  }

  .tv-left {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 18px;
    min-width: 0;
  }

  .tv-cover-wrap {
    position: relative;
    width: clamp(220px, 32vh, 360px);
    height: clamp(220px, 32vh, 360px);
    border-radius: 26px;
    box-shadow:
      0 28px 70px rgba(0, 0, 0, 0.55),
      0 0 80px color-mix(in srgb, var(--accent, #6366f1) 35%, transparent);
    overflow: hidden;
  }

  .tv-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .tv-meta {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    max-width: 100%;
  }

  .tv-title {
    margin: 0;
    font-size: clamp(22px, 2.8vw, 34px);
    font-weight: 800;
    letter-spacing: -0.02em;
    color: var(--text-primary);
    line-height: 1.15;
    max-width: 560px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tv-artist-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 16px;
  }

  .tv-sub {
    font-size: 13px;
    color: var(--text-secondary);
    opacity: 0.85;
  }

  .tv-controls {
    width: min(460px, 100%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    transition: opacity 0.45s ease;
  }

  .tv-transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }

  .tv-stage.tv-idle .tv-controls,
  .tv-stage.tv-idle .tv-fs-btn {
    opacity: 0.14;
  }

  .tv-stage.tv-idle .tv-controls:hover,
  .tv-stage.tv-idle .tv-fs-btn:hover {
    opacity: 1;
  }

  .tv-right {
    height: 100%;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .tv-lyrics-wrap {
    width: 100%;
    height: min(520px, 72vh);
    min-height: 0;
  }

  .tv-lounge {
    width: 100%;
    max-width: 460px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .tv-clock {
    font-size: clamp(56px, 8vw, 96px);
    font-weight: 800;
    line-height: 0.95;
    letter-spacing: -0.04em;
    font-variant-numeric: tabular-nums;
    color: var(--text-primary);
  }

  .tv-date {
    font-size: clamp(16px, 1.8vw, 22px);
    font-weight: 500;
    color: var(--text-secondary);
    text-transform: capitalize;
  }

  .tv-weather {
    font-size: 14px;
    font-weight: 600;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .tv-upnext {
    margin-top: 14px;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(16px);
    text-align: left;
    cursor: pointer;
    transition: transform 0.18s ease, background 0.18s ease;
  }

  .tv-upnext:hover {
    transform: translateY(-2px);
    background: rgba(255, 255, 255, 0.1);
  }

  .tv-upnext img {
    width: 56px;
    height: 56px;
    border-radius: 12px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .tv-un-lbl {
    display: block;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent, #818cf8);
    margin-bottom: 2px;
  }

  .tv-un-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .tv-un-art {
    font-size: 13px;
    color: var(--text-secondary);
  }

  @media (max-width: 860px) {
    .tv-stage {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }
    .tv-lounge {
      align-items: center;
      text-align: center;
    }
  }
</style>

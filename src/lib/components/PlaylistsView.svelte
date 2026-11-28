<script lang="ts">
  import { Clock, Heart, ListMusic, Play, Plus, Search, Shuffle, Trash2, X } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import type { Playlist, SourceKind, Track } from '../types';

  interface Props {
    playlists: Playlist[];
    currentTrackId?: string | null;
    isPlaying?: boolean;
    likedIds?: Set<string>;
    onCreatePlaylist: (title: string) => Promise<void>;
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onRemoveTrack: (playlistId: string, trackId: string) => Promise<void>;
    onToggleLike?: (track: Track) => void;
    onOpenArtist?: (artist: string) => void;
  }

  let {
    playlists,
    currentTrackId = null,
    isPlaying = false,
    likedIds = new Set(),
    onCreatePlaylist,
    onPlayTrack,
    onRemoveTrack,
    onToggleLike,
    onOpenArtist,
  }: Props = $props();

  type PlaylistFilter = 'all' | 'local' | 'yandex' | 'soundcloud';

  let newTitle = $state('');
  let showCreate = $state(false);
  let playlistQuery = $state('');
  let playlistFilter = $state<PlaylistFilter>('all');
  let trackQuery = $state('');
  let selectedId = $state<string | null>(null);

  $effect(() => {
    if (!selectedId && playlists.length > 0) {
      selectedId = playlists[0].id;
    }
  });

  const filteredPlaylists = $derived.by(() => {
    const q = playlistQuery.trim().toLowerCase();
    return playlists.filter((pl) => {
      if (playlistFilter === 'local' && !pl.isLocal) return false;
      if (playlistFilter === 'yandex' && (pl.isLocal || pl.source !== 'yandex')) return false;
      if (playlistFilter === 'soundcloud' && (pl.isLocal || pl.source !== 'soundcloud')) return false;
      if (q && !pl.title.toLowerCase().includes(q)) return false;
      return true;
    });
  });

  const activePlaylist = $derived(
    playlists.find((p) => p.id === selectedId) ?? filteredPlaylists[0] ?? playlists[0] ?? null
  );

  const visibleTracks = $derived.by(() => {
    if (!activePlaylist) return [];
    const q = trackQuery.trim().toLowerCase();
    if (!q) return activePlaylist.tracks;
    return activePlaylist.tracks.filter(
      (t) => t.title.toLowerCase().includes(q) || t.artist.toLowerCase().includes(q)
    );
  });

  const totalDurationLabel = $derived.by(() => {
    if (!activePlaylist) return '';
    const totalSec = activePlaylist.tracks.reduce((acc, t) => acc + (t.duration || 0), 0);
    if (totalSec <= 0) return '';
    const mins = Math.round(totalSec / 60);
    if (mins < 60) return `${mins} мин`;
    const hrs = Math.floor(mins / 60);
    const rem = mins % 60;
    return rem > 0 ? `${hrs} ч ${rem} мин` : `${hrs} ч`;
  });

  function formatDuration(sec: number): string {
    if (!sec || sec <= 0) return '0:00';
    const s = Math.floor(sec);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const r = s % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
    return `${m}:${String(r).padStart(2, '0')}`;
  }

  function pluralTracks(n: number): string {
    const mod10 = n % 10;
    const mod100 = n % 100;
    if (mod10 === 1 && mod100 !== 11) return `${n} трек`;
    if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return `${n} трека`;
    return `${n} треков`;
  }

  function getCollageCovers(pl: Playlist): string[] {
    if (pl.artwork) return [pl.artwork];
    const seen = new Set<string>();
    const res: string[] = [];
    for (const t of pl.tracks) {
      if (t.artwork && !seen.has(t.artwork)) {
        seen.add(t.artwork);
        res.push(t.artwork);
        if (res.length === 4) break;
      }
    }
    return res;
  }

  function sourceLabel(src: SourceKind | null, isLocal: boolean): string {
    if (isLocal) return 'Локальная подборка';
    if (src === 'yandex') return 'Яндекс Музыка';
    if (src === 'soundcloud') return 'SoundCloud';
    return 'Плейлист';
  }

  function sourceDotColor(src: SourceKind | null, isLocal: boolean): string {
    if (isLocal) return 'var(--accent)';
    if (src === 'yandex') return '#fcca00';
    if (src === 'soundcloud') return '#f97316';
    return '#a855f7';
  }

  async function handleCreate(e: Event) {
    e.preventDefault();
    if (!newTitle.trim()) return;
    await onCreatePlaylist(newTitle.trim());
    newTitle = '';
    showCreate = false;
  }

  function handleShufflePlay() {
    if (!activePlaylist || activePlaylist.tracks.length === 0) return;
    const copy = [...activePlaylist.tracks];
    for (let i = copy.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [copy[i], copy[j]] = [copy[j], copy[i]];
    }
    onPlayTrack(copy[0], copy);
  }
</script>

<section class="playlists-view tab-enter">
  <aside class="pl-sidebar">
    <div class="pl-side-head">
      <div class="pl-side-title-row">
        <h3>Плейлисты</h3>
        <button
          type="button"
          class="pl-add-toggle"
          class:active={showCreate}
          onclick={() => (showCreate = !showCreate)}
          title="Создать локальный плейлист"
          aria-label="Создать локальный плейлист"
        >
          <Plus size={15} />
        </button>
      </div>

      {#if showCreate}
        <form class="create-form" onsubmit={handleCreate}>
          <input type="text" bind:value={newTitle} placeholder="Название подборки..." />
          <button type="submit" aria-label="Создать">Создать</button>
        </form>
      {/if}

      <div class="pl-side-search">
        <Search size={13} />
        <input type="text" bind:value={playlistQuery} placeholder="Фильтр плейлистов..." />
        {#if playlistQuery}
          <button type="button" class="clear-x" onclick={() => (playlistQuery = '')} aria-label="Очистить">
            <X size={12} />
          </button>
        {/if}
      </div>

      <div class="pl-filter-pills">
        <button type="button" class:active={playlistFilter === 'all'} onclick={() => (playlistFilter = 'all')}>Все</button>
        <button type="button" class:active={playlistFilter === 'local'} onclick={() => (playlistFilter = 'local')}>Мои</button>
        <button type="button" class:active={playlistFilter === 'yandex'} onclick={() => (playlistFilter = 'yandex')}>Яндекс</button>
        <button type="button" class:active={playlistFilter === 'soundcloud'} onclick={() => (playlistFilter = 'soundcloud')}>SC</button>
      </div>
    </div>

    <div class="pl-list">
      {#each filteredPlaylists as pl, pIdx (`${pl.id}-${pIdx}`)}
        {@const covers = getCollageCovers(pl)}
        <button
          type="button"
          class="pl-item"
          class:active={activePlaylist?.id === pl.id}
          onclick={() => {
            selectedId = pl.id;
            trackQuery = '';
          }}
        >
          <div class="pl-thumb" class:quad={covers.length === 4}>
            {#if covers.length === 4}
              {#each covers as c}
                <img src={c} alt="" />
              {/each}
            {:else if covers.length > 0}
              <img src={covers[0]} alt="" />
            {:else}
              <div class="pl-thumb-empty">
                <ListMusic size={16} />
              </div>
            {/if}
          </div>
          <div class="pl-item-meta">
            <span class="pl-name">{pl.title}</span>
            <span class="pl-sub">
              <i class="pl-dot" style="background: {sourceDotColor(pl.source, pl.isLocal)}"></i>
              <span>{pluralTracks(pl.tracks.length)}</span>
            </span>
          </div>
        </button>
      {/each}
      {#if filteredPlaylists.length === 0}
        <div class="pl-side-empty">Ничего не найдено</div>
      {/if}
    </div>
  </aside>

  <div class="pl-content">
    {#if activePlaylist}
      {@const heroCovers = getCollageCovers(activePlaylist)}
      <div class="pl-hero">
        {#if heroCovers.length > 0}
          <div class="pl-hero-glow" style="background-image: url({heroCovers[0]})"></div>
        {/if}

        <div class="pl-hero-cover" class:quad={heroCovers.length === 4}>
          {#if heroCovers.length === 4}
            {#each heroCovers as c}
              <img src={c} alt="" />
            {/each}
          {:else if heroCovers.length > 0}
            <img src={heroCovers[0]} alt={activePlaylist.title} />
          {:else}
            <div class="pl-hero-empty">
              <ListMusic size={44} />
            </div>
          {/if}
        </div>

        <div class="pl-hero-info">
          <div class="pl-hero-kicker">
            <i class="pl-dot" style="background: {sourceDotColor(activePlaylist.source, activePlaylist.isLocal)}"></i>
            <span>{sourceLabel(activePlaylist.source, activePlaylist.isLocal)}</span>
          </div>
          <h2>{activePlaylist.title}</h2>
          <p class="pl-hero-desc">
            {activePlaylist.description ||
              (activePlaylist.isLocal
                ? 'Локальная мульти-сервисная подборка (Яндекс + SoundCloud)'
                : 'Плейлист подключённого стримингового сервиса')}
          </p>
          <div class="pl-hero-stats">
            <span>{pluralTracks(activePlaylist.tracks.length)}</span>
            {#if totalDurationLabel}
              <span>·</span>
              <span>{totalDurationLabel}</span>
            {/if}
          </div>

          <div class="pl-hero-actions">
            {#if activePlaylist.tracks.length > 0}
              <button
                type="button"
                class="btn-hero-play"
                onclick={() => onPlayTrack(activePlaylist.tracks[0], activePlaylist.tracks)}
              >
                <Play size={14} fill="currentColor" />
                <span>Слушать</span>
              </button>
              <button
                type="button"
                class="btn-hero-ghost"
                onclick={handleShufflePlay}
                title="Перемешать и запустить"
              >
                <Shuffle size={14} />
                <span>Перемешать</span>
              </button>
            {/if}

            <div class="pl-track-search">
              <Search size={13} />
              <input type="text" bind:value={trackQuery} placeholder="Поиск в плейлисте..." />
              {#if trackQuery}
                <button type="button" class="clear-x" onclick={() => (trackQuery = '')} aria-label="Очистить">
                  <X size={12} />
                </button>
              {/if}
            </div>
          </div>
        </div>
      </div>

      {#if activePlaylist.tracks.length === 0}
        <div class="pl-empty">
          В этом плейлисте пока нет треков. Добавляйте треки из Библиотеки или Главного экрана с помощью кнопки «+».
        </div>
      {:else if visibleTracks.length === 0}
        <div class="pl-empty">По запросу «{trackQuery}» треки не найдены.</div>
      {:else}
        <div class="pl-table-head">
          <span class="col-num">#</span>
          <span class="col-title">Трек</span>
          <span class="col-src">Источник</span>
          <span class="col-dur" title="Длительность"><Clock size={13} /></span>
          <span class="col-act"></span>
        </div>

        <div class="pl-tracks">
          {#each visibleTracks as track, tIdx (`${track.id}-${tIdx}`)}
            {@const isCurrent = currentTrackId === track.id}
            {@const isLiked = likedIds.has(track.id)}
            <div class="pl-row" class:current={isCurrent}>
              <button
                type="button"
                class="col-num-btn"
                onclick={() => onPlayTrack(track, visibleTracks)}
                aria-label={`Воспроизвести ${track.title}`}
              >
                {#if isCurrent && isPlaying}
                  <span class="eq-bars" aria-hidden="true"><i></i><i></i><i></i></span>
                {:else}
                  <span class="num-txt">{tIdx + 1}</span>
                  <span class="num-play"><Play size={13} fill="currentColor" /></span>
                {/if}
              </button>

              <div class="col-track">
                <button
                  type="button"
                  class="pl-cover-btn"
                  onclick={() => onPlayTrack(track, visibleTracks)}
                >
                  <img src={track.artwork} alt={track.title} loading="lazy" />
                </button>
                <div class="pl-meta">
                  <button
                    type="button"
                    class="pl-t"
                    onclick={() => onPlayTrack(track, visibleTracks)}
                  >
                    {track.title}
                  </button>
                  <button
                    type="button"
                    class="pl-a"
                    onclick={() => onOpenArtist?.(track.artist)}
                  >
                    {track.artist}
                  </button>
                </div>
              </div>

              <div class="col-src">
                <SourceBadge source={track.source} />
              </div>

              <div class="col-dur">{formatDuration(track.duration)}</div>

              <div class="col-act">
                {#if onToggleLike}
                  <button
                    type="button"
                    class="row-icon-btn"
                    class:liked={isLiked}
                    onclick={() => onToggleLike(track)}
                    title={isLiked ? 'Убрать из избранного' : 'Нравится'}
                    aria-label="Лайк"
                  >
                    <Heart size={14} fill={isLiked ? 'currentColor' : 'none'} />
                  </button>
                {/if}
                {#if activePlaylist.isLocal}
                  <button
                    type="button"
                    class="row-icon-btn del"
                    title="Удалить из подборки"
                    onclick={() => onRemoveTrack(activePlaylist.id, track.id)}
                    aria-label="Удалить из подборки"
                  >
                    <Trash2 size={14} />
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="pl-empty">Создайте первый локальный плейлист слева или подключите сервис в Настройках.</div>
    {/if}
  </div>
</section>

<style>
  .playlists-view {
    height: 100%;
    display: grid;
    grid-template-columns: clamp(276px, 21vw, 336px) 1fr;
    gap: 0;
  }

  .pl-sidebar {
    border-right: 1px solid var(--border-subtle);
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: rgba(12, 12, 16, 0.45);
    min-height: 0;
  }

  .pl-side-head {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .pl-side-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px;
  }

  .pl-side-title-row h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .pl-add-toggle {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: all 0.14s ease;
  }

  .pl-add-toggle:hover,
  .pl-add-toggle.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .create-form {
    display: flex;
    gap: 6px;
  }

  .create-form input {
    flex: 1;
    min-width: 0;
    height: 32px;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-primary);
    padding: 0 10px;
    font-size: 12px;
    outline: none;
  }

  .create-form input:focus {
    border-color: var(--accent);
  }

  .create-form button {
    height: 32px;
    padding: 0 11px;
    border-radius: 8px;
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .pl-side-search,
  .pl-track-search {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-muted);
  }

  .pl-side-search input,
  .pl-track-search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
  }

  .clear-x {
    border: none;
    background: transparent;
    color: var(--text-muted);
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 2px;
  }

  .clear-x:hover {
    color: var(--text-primary);
  }

  .pl-filter-pills {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }

  .pl-filter-pills button {
    height: 26px;
    border-radius: 7px;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .pl-filter-pills button.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--text-primary);
  }

  .pl-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
    padding-right: 2px;
    flex: 1;
  }

  .pl-item {
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    border-radius: 10px;
    padding: 6px 8px;
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    text-align: left;
    transition: background 0.14s ease, border-color 0.14s ease;
  }

  .pl-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-primary);
  }

  .pl-item.active {
    background: color-mix(in srgb, var(--accent) 14%, rgba(255, 255, 255, 0.04));
    border-color: color-mix(in srgb, var(--accent) 32%, transparent);
    color: var(--text-primary);
  }

  .pl-thumb {
    width: 38px;
    height: 38px;
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.06);
  }

  .pl-thumb.quad {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
  }

  .pl-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .pl-thumb-empty {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.02));
    color: var(--text-muted);
  }

  .pl-item-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pl-name {
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pl-sub {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .pl-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .pl-side-empty {
    padding: 20px 10px;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
  }

  .pl-content {
    padding: 20px 26px 32px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  .pl-hero {
    position: relative;
    overflow: hidden;
    border-radius: 16px;
    padding: 20px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.028);
    display: flex;
    align-items: center;
    gap: 20px;
  }

  .pl-hero-glow {
    position: absolute;
    inset: -20%;
    background-size: cover;
    background-position: center;
    filter: blur(55px) saturate(1.4);
    opacity: 0.22;
    pointer-events: none;
  }

  .pl-hero-cover {
    position: relative;
    z-index: 1;
    width: clamp(140px, 11vw, 176px);
    height: clamp(140px, 11vw, 176px);
    border-radius: 14px;
    overflow: hidden;
    flex-shrink: 0;
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.45);
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
  }

  .pl-hero-cover.quad {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
  }

  .pl-hero-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .pl-hero-empty {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 18%, transparent), rgba(255, 255, 255, 0.04));
  }

  .pl-hero-info {
    position: relative;
    z-index: 1;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pl-hero-kicker {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }

  .pl-hero-info h2 {
    margin: 0;
    font-size: clamp(24px, 2.1vw, 34px);
    font-weight: 800;
    color: var(--text-primary);
    line-height: 1.15;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pl-hero-desc {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pl-hero-stats {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .pl-hero-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
  }

  .btn-hero-play,
  .btn-hero-ghost {
    height: 34px;
    border-radius: 999px;
    padding: 0 16px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .btn-hero-play {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 6px 18px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .btn-hero-play:hover {
    filter: brightness(1.08);
  }

  .btn-hero-ghost {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .btn-hero-ghost:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .pl-track-search {
    margin-left: auto;
    width: 210px;
    height: 34px;
    border-radius: 999px;
  }

  .pl-table-head {
    display: grid;
    grid-template-columns: 36px 1fr 140px 56px 64px;
    align-items: center;
    gap: 10px;
    padding: 4px 12px 6px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .col-num,
  .col-dur {
    text-align: center;
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .pl-tracks {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pl-row {
    display: grid;
    grid-template-columns: 36px 1fr 140px 56px 64px;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border-radius: 10px;
    transition: background 0.13s ease;
  }

  .pl-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .pl-row.current {
    background: color-mix(in srgb, var(--accent) 12%, rgba(255, 255, 255, 0.03));
  }

  .col-num-btn {
    border: none;
    background: transparent;
    color: var(--text-muted);
    height: 32px;
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0;
  }

  .num-play {
    display: none;
    color: var(--text-primary);
  }

  .pl-row:hover .num-txt {
    display: none;
  }

  .pl-row:hover .num-play {
    display: grid;
    place-items: center;
  }

  .pl-row.current .num-txt {
    color: var(--accent);
    font-weight: 700;
  }

  .eq-bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
  }

  .eq-bars i {
    width: 2.5px;
    border-radius: 999px;
    background: var(--accent);
    animation: pl-eq 0.75s ease-in-out infinite alternate;
  }

  .eq-bars i:nth-child(1) {
    height: 6px;
  }

  .eq-bars i:nth-child(2) {
    height: 12px;
    animation-delay: 0.2s;
  }

  .eq-bars i:nth-child(3) {
    height: 8px;
    animation-delay: 0.4s;
  }

  @keyframes pl-eq {
    from {
      transform: scaleY(0.35);
    }
    to {
      transform: scaleY(1);
    }
  }

  .col-track {
    display: flex;
    align-items: center;
    gap: 11px;
    min-width: 0;
  }

  .pl-cover-btn {
    width: clamp(40px, 3.1vw, 48px);
    height: clamp(40px, 3.1vw, 48px);
    border-radius: 8px;
    border: none;
    padding: 0;
    overflow: hidden;
    flex-shrink: 0;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.06);
  }

  .pl-cover-btn img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .pl-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }

  .pl-t,
  .pl-a {
    border: none;
    background: transparent;
    padding: 0;
    text-align: left;
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .pl-t {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .pl-row.current .pl-t {
    color: var(--accent);
  }

  .pl-a {
    font-size: 11.5px;
    color: var(--text-muted);
  }

  .pl-a:hover {
    color: var(--text-primary);
    text-decoration: underline;
  }

  .col-src {
    display: flex;
    align-items: center;
  }

  .col-dur {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .col-act {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }

  .row-icon-btn {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s ease, background 0.12s ease, color 0.12s ease;
  }

  .pl-row:hover .row-icon-btn,
  .row-icon-btn.liked {
    opacity: 1;
  }

  .row-icon-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .row-icon-btn.liked {
    color: #fb7185;
  }

  .row-icon-btn.del:hover {
    color: #fb7185;
  }

  .pl-empty {
    padding: 32px 24px;
    border: 1px dashed var(--border-subtle);
    border-radius: 14px;
    color: var(--text-secondary);
    text-align: center;
    font-size: 13px;
  }
</style>

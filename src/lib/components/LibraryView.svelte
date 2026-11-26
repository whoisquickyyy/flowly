<script lang="ts">
  import { Play, Plus, ExternalLink, Heart, Search, FolderPlus, RefreshCw, X } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import { fuzzySearchTracks } from '../fuzzySearch';
  import type { LibrarySection, Playlist, SourceKind, Track } from '../types';

  interface Props {
    currentTrackId: string | null;
    localPlaylists: Playlist[];
    likedIds?: Set<string>;
    enabledSources?: SourceKind[];
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onAddToPlaylist: (playlistId: string, track: Track) => void;
    onToggleLike?: (track: Track) => void;
    onOpenArtist?: (artist: string) => void;
  }

  let {
    currentTrackId,
    localPlaylists,
    likedIds = new Set(),
    enabledSources = ['yandex', 'spotify', 'soundcloud'],
    onPlayTrack,
    onAddToPlaylist,
    onToggleLike,
    onOpenArtist,
  }: Props = $props();

  let section = $state<LibrarySection>('allTracks');
  let sourceFilter = $state<SourceKind | null>(null);
  let tracks = $state<Track[]>([]);
  let loading = $state(false);
  let selectedPlaylistId = $state<string>('');
  let selectedAlbum = $state<string | null>(null);
  let libQuery = $state('');
  let localFolders = $state<string[]>([]);
  let scanningLocal = $state(false);

  const groupedAlbums = $derived.by(() => {
    const map = new Map<string, { title: string; artist: string; artwork: string; source: SourceKind; tracks: Track[] }>();
    for (const t of tracks) {
      const key = t.album || 'Single';
      const existing = map.get(key);
      if (existing) {
        existing.tracks.push(t);
      } else {
        map.set(key, {
          title: key,
          artist: t.artist,
          artwork: t.artwork,
          source: t.source,
          tracks: [t],
        });
      }
    }
    return Array.from(map.values());
  });

  const groupedArtists = $derived.by(() => {
    const map = new Map<string, { name: string; artwork: string; tracks: Track[] }>();
    for (const t of tracks) {
      const name = t.artist.split(/[,&]/)[0]?.trim() || t.artist;
      const existing = map.get(name);
      if (existing) {
        existing.tracks.push(t);
      } else {
        map.set(name, {
          name,
          artwork: t.artwork,
          tracks: [t],
        });
      }
    }
    return Array.from(map.values());
  });

  const displayedTracks = $derived.by(() => {
    const base =
      section === 'albums' && selectedAlbum
        ? tracks.filter((t) => (t.album || 'Single') === selectedAlbum)
        : tracks;
    if (libQuery.trim()) {
      return fuzzySearchTracks(libQuery, base, 120);
    }
    return base;
  });

  const sections: { id: LibrarySection; label: string }[] = [
    { id: 'allTracks', label: 'Все треки' },
    { id: 'albums', label: 'Альбомы' },
    { id: 'artists', label: 'Исполнители' },
    { id: 'likedSongs', label: 'Любимые' },
    { id: 'recentlyPlayed', label: 'Недавние' },
    { id: 'localFiles', label: 'Локальные файлы' },
  ];

  $effect(() => {
    if (!selectedPlaylistId && localPlaylists.length > 0) {
      selectedPlaylistId = localPlaylists[0].id;
    }
  });

  async function refreshLocalFoldersAndTracks() {
    scanningLocal = true;
    try {
      const [folders, scanned] = await Promise.all([
        api.getLocalFolders(),
        api.scanLocalTracks(),
      ]);
      localFolders = folders;
      tracks = scanned;
    } finally {
      scanningLocal = false;
    }
  }

  async function handleAddLocalFolder() {
    scanningLocal = true;
    try {
      localFolders = await api.pickAndAddLocalFolder();
      tracks = await api.scanLocalTracks();
    } finally {
      scanningLocal = false;
    }
  }

  async function handleRemoveLocalFolder(path: string) {
    scanningLocal = true;
    try {
      localFolders = await api.removeLocalFolder(path);
      tracks = await api.scanLocalTracks();
    } finally {
      scanningLocal = false;
    }
  }

  async function load() {
    loading = true;
    try {
      if (section === 'localFiles') {
        await refreshLocalFoldersAndTracks();
        return;
      }
      const effectiveFilter =
        sourceFilter && enabledSources.includes(sourceFilter)
          ? sourceFilter
          : enabledSources.length === 1
            ? enabledSources[0]
            : null;
      const res = await api.getLibrarySection(section, effectiveFilter);
      tracks = res.filter((t) => enabledSources.includes(t.source));
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    section;
    sourceFilter;
    load();
  });

  function formatDuration(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<section class="library-view">
  <div class="lib-header">
    <div class="section-tabs">
      {#each sections as sec (sec.id)}
        <button
          class="tab-btn"
          class:active={section === sec.id}
          onclick={() => {
            section = sec.id;
            selectedAlbum = null;
          }}
        >
          {sec.label}
        </button>
      {/each}
    </div>

    <div class="lib-fuzzy-box">
      <Search size={13} />
      <input
        type="text"
        bind:value={libQuery}
        placeholder="Умный поиск (напр. дефт панк)..."
      />
    </div>

    {#if section !== 'localFiles' && enabledSources.length > 1}
      <div class="source-filters">
        <button
          class="filter-chip"
          class:active={sourceFilter === null}
          onclick={() => (sourceFilter = null)}
        >
          Все
        </button>
        {#if enabledSources.includes('yandex')}
          <button
            class="filter-chip"
            class:active={sourceFilter === 'yandex'}
            onclick={() => (sourceFilter = 'yandex')}
          >
            Яндекс Музыка
          </button>
        {/if}
        {#if enabledSources.includes('spotify')}
          <button
            class="filter-chip"
            class:active={sourceFilter === 'spotify'}
            onclick={() => (sourceFilter = 'spotify')}
          >
            Spotify
          </button>
        {/if}
        {#if enabledSources.includes('soundcloud')}
          <button
            class="filter-chip"
            class:active={sourceFilter === 'soundcloud'}
            onclick={() => (sourceFilter = 'soundcloud')}
          >
            SoundCloud
          </button>
        {/if}
      </div>
    {/if}
  </div>

  {#if section === 'localFiles'}
    <div class="local-bar">
      <div class="local-bar-actions">
        <button type="button" class="btn-add-folder" disabled={scanningLocal} onclick={handleAddLocalFolder}>
          <FolderPlus size={14} />
          <span>Добавить папку</span>
        </button>
        <button type="button" class="filter-chip" disabled={scanningLocal} onclick={refreshLocalFoldersAndTracks}>
          <RefreshCw size={12} class={scanningLocal ? 'spin' : ''} />
          <span>{scanningLocal ? 'Сканирование...' : `Обновить (${tracks.length})`}</span>
        </button>
      </div>
      {#if localFolders.length > 0}
        <div class="local-folders-list">
          {#each localFolders as folder (folder)}
            <span class="folder-pill" title={folder}>
              <span class="folder-path">{folder}</span>
              <button type="button" class="folder-del" onclick={() => handleRemoveLocalFolder(folder)} aria-label="Удалить папку">
                <X size={11} />
              </button>
            </span>
          {/each}
        </div>
      {:else}
        <div class="local-hint">Добавьте папку с MP3, FLAC, WAV, OGG или M4A — треки появятся здесь с обложками и тегами.</div>
      {/if}
    </div>
  {/if}

  {#if localPlaylists.length > 0}
    <div class="playlist-target">
      <span>Быстрое добавление в подборку:</span>
      <select bind:value={selectedPlaylistId}>
        {#each localPlaylists as pl (pl.id)}
          <option value={pl.id}>{pl.title}</option>
        {/each}
      </select>
    </div>
  {/if}

  {#if loading}
    <div class="state-msg">Загрузка медиатеки...</div>
  {:else if tracks.length === 0}
    <div class="state-msg">Треки не найдены. Подключите сервисы в Настройках.</div>
  {:else if section === 'albums' && !selectedAlbum}
    <div class="cards-grid">
      {#each groupedAlbums as alb (alb.title)}
        <button type="button" class="lib-card" onclick={() => (selectedAlbum = alb.title)}>
          <img src={alb.artwork} alt={alb.title} loading="lazy" />
          <div class="lc-title">{alb.title}</div>
          <div class="lc-sub">{alb.artist} · {alb.tracks.length} трек.</div>
        </button>
      {/each}
    </div>
  {:else if section === 'artists'}
    <div class="cards-grid">
      {#each groupedArtists as art (art.name)}
        <button type="button" class="lib-card artist-card" onclick={() => onOpenArtist?.(art.name)}>
          <img src={art.artwork} alt={art.name} loading="lazy" />
          <div class="lc-title">{art.name}</div>
          <div class="lc-sub">{art.tracks.length} трек.</div>
        </button>
      {/each}
    </div>
  {:else}
    {#if section === 'albums' && selectedAlbum}
      <div class="album-bar">
        <button type="button" class="filter-chip" onclick={() => (selectedAlbum = null)}>
          ← Все альбомы
        </button>
        <span class="album-bar-title">{selectedAlbum}</span>
      </div>
    {/if}
    <div class="tracks-table">
      {#each displayedTracks as track, idx (track.id)}
        {@const active = currentTrackId === track.id}
        <div
          class="track-row"
          class:active
          role="button"
          tabindex="0"
          onclick={() => onPlayTrack(track, displayedTracks)}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onPlayTrack(track, displayedTracks);
            }
          }}
        >
          <span class="idx">{idx + 1}</span>
          <button
            type="button"
            class="play-row-btn"
            onclick={(e) => {
              e.stopPropagation();
              onPlayTrack(track, displayedTracks);
            }}
            aria-label="Слушать"
          >
            <img src={track.artwork} alt={track.title} />
            <span class="play-hover"><Play size={14} fill="currentColor" /></span>
          </button>

          <div class="main-col">
            <span class="title-btn">{track.title}</span>
            <button
              type="button"
              class="artist-btn"
              onclick={(e) => {
                e.stopPropagation();
                onOpenArtist?.(track.artist);
              }}
              title="Открыть артиста"
            >
              {track.artist}
            </button>
          </div>

          <div class="album-col">{track.album}</div>

          <div class="source-col">
            <SourceBadge source={track.source} />
          </div>

          <div class="dur-col">{formatDuration(track.duration)}</div>

          <div class="row-actions">
            {#if onToggleLike}
              {@const liked = likedIds.has(track.id)}
              <button
                type="button"
                class="icon-action"
                class:liked
                title={liked ? 'Убрать лайк' : 'Лайкнуть'}
                onclick={(e) => {
                  e.stopPropagation();
                  onToggleLike(track);
                }}
              >
                <Heart size={14} fill={liked ? 'currentColor' : 'none'} />
              </button>
            {/if}
            {#if selectedPlaylistId}
              <button
                type="button"
                class="icon-action"
                title="Добавить в локальный плейлист"
                onclick={(e) => {
                  e.stopPropagation();
                  onAddToPlaylist(selectedPlaylistId, track);
                }}
              >
                <Plus size={14} />
              </button>
            {/if}
            <button
              type="button"
              class="icon-action"
              title="Открыть в оригинальном сервисе"
              onclick={(e) => {
                e.stopPropagation();
                api.openExternalUrl(track.externalUrl);
              }}
            >
              <ExternalLink size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .library-view {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 28px;
  }

  .lib-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 12px;
  }

  .section-tabs,
  .source-filters {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .tab-btn,
  .filter-chip {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.14s ease, color 0.14s ease;
  }

  .tab-btn:hover,
  .filter-chip:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .tab-btn.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }

  .filter-chip.active {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .playlist-target {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .playlist-target select {
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    padding: 4px 10px;
    font-size: 12px;
  }

  .tracks-table {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .track-row {
    display: grid;
    grid-template-columns: 28px 42px 1.6fr 1.1fr 140px 56px 64px;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .track-row:hover {
    background: var(--bg-surface);
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .idx {
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .play-row-btn {
    position: relative;
    width: 40px;
    height: 40px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: none;
    padding: 0;
    cursor: pointer;
    background: var(--bg-surface);
  }

  .play-row-btn img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .play-hover {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    color: white;
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 0.12s ease;
  }

  .track-row:hover .play-hover {
    opacity: 1;
  }

  .main-col {
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .title-btn {
    border: none;
    background: transparent;
    padding: 0;
    text-align: left;
    cursor: pointer;
    max-width: 100%;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist-btn {
    border: none;
    background: transparent;
    padding: 0;
    text-align: left;
    cursor: pointer;
    max-width: 100%;
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist-btn:hover {
    color: var(--text-primary);
    text-decoration: underline;
  }

  .icon-action.liked {
    color: #ff3b5c;
  }

  .album-col {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dur-col {
    font-size: 12px;
    color: var(--text-muted);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }

  .icon-action {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .icon-action:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .state-msg {
    padding: 40px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 14px;
  }

  .lib-card {
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.025);
    border-radius: 14px;
    padding: 10px;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 7px;
    cursor: pointer;
    transition: transform 0.14s ease, background-color 0.14s ease, border-color 0.14s ease;
  }

  .lib-card:hover {
    transform: translateY(-2px);
    background: rgba(255, 255, 255, 0.055);
    border-color: var(--border-strong);
  }

  .lib-card img {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 10px;
    object-fit: cover;
  }

  .lib-card.artist-card img {
    border-radius: 50%;
  }

  .lc-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .lc-sub {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .album-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .album-bar-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .lib-fuzzy-box {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    min-width: 220px;
  }

  .lib-fuzzy-box input {
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    width: 100%;
  }

  .local-bar {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 14px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .local-bar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .btn-add-folder {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 13px;
    border-radius: 10px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .local-folders-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .folder-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-subtle);
    font-size: 11px;
    color: var(--text-secondary);
    max-width: 340px;
  }

  .folder-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-del {
    border: none;
    background: transparent;
    color: var(--text-muted);
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 1px;
  }

  .folder-del:hover {
    color: #ef4444;
  }

  .local-hint {
    font-size: 12px;
    color: var(--text-secondary);
  }
</style>

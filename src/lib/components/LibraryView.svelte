<script lang="ts">
  import { Play, Plus, ExternalLink, Heart, Search, FolderPlus, RefreshCw, X, LayoutGrid, List } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import { fuzzySearchTracks } from '../fuzzySearch';
  import type { AuthStatus, LibrarySection, Playlist, SourceKind, Track } from '../types';

  interface Props {
    currentTrackId: string | null;
    localPlaylists: Playlist[];
    likedIds?: Set<string>;
    enabledSources?: SourceKind[];
    authStatuses?: AuthStatus[];
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onAddToPlaylist: (playlistId: string, track: Track) => void;
    onToggleLike?: (track: Track) => void;
    onOpenArtist?: (artist: string) => void;
    onOpenAuth?: () => void;
  }

  let {
    currentTrackId,
    localPlaylists,
    likedIds = new Set(),
    enabledSources = ['yandex', 'soundcloud'],
    authStatuses = [],
    onPlayTrack,
    onAddToPlaylist,
    onToggleLike,
    onOpenArtist,
    onOpenAuth,
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
  let viewMode = $state<'grid' | 'list'>('grid');
  let visibleLimit = $state(90);
  let loadToken = 0;
  const sectionCache = new Map<string, { ts: number; tracks: Track[] }>();

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

  const renderedTracks = $derived(displayedTracks.slice(0, visibleLimit));

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

  async function load(force = false) {
    const token = ++loadToken;
    visibleLimit = 90;
    if (section === 'localFiles') {
      loading = true;
      try {
        await refreshLocalFoldersAndTracks();
      } finally {
        if (token === loadToken) loading = false;
      }
      return;
    }

    const effectiveFilter =
      sourceFilter && enabledSources.includes(sourceFilter)
        ? sourceFilter
        : enabledSources.length === 1
          ? enabledSources[0]
          : null;
    const cacheKey = `${section}:${effectiveFilter ?? 'all'}`;
    const now = Date.now();

    if (!force) {
      const hit = sectionCache.get(cacheKey);
      if (hit && now - hit.ts < 180_000) {
        tracks = hit.tracks.filter((t) => enabledSources.includes(t.source));
        loading = false;
        return;
      }
      // Если уже загружены все треки этой же секции — фильтруем по сервису мгновенно без сети
      if (effectiveFilter) {
        const allHit = sectionCache.get(`${section}:all`);
        if (allHit && now - allHit.ts < 180_000) {
          tracks = allHit.tracks.filter((t) => enabledSources.includes(t.source) && t.source === effectiveFilter);
          loading = false;
          return;
        }
      }
      if (section === 'artists') {
        const allHit = sectionCache.get(`allTracks:${effectiveFilter ?? 'all'}`);
        if (allHit && now - allHit.ts < 180_000) {
          tracks = allHit.tracks.filter((t) => enabledSources.includes(t.source));
          loading = false;
          return;
        }
      }
    }

    loading = true;
    try {
      const res = await api.getLibrarySection(section, effectiveFilter);
      if (token !== loadToken) return;
      const filtered = res.filter((t) => enabledSources.includes(t.source));
      sectionCache.set(cacheKey, { ts: Date.now(), tracks: filtered });
      tracks = filtered;
    } finally {
      if (token === loadToken) loading = false;
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

    <div class="lib-view-actions">
      <div class="view-toggle">
        <button
          type="button"
          class:active={viewMode === 'grid'}
          onclick={() => (viewMode = 'grid')}
          title="Сетка обложек"
          aria-label="Сетка"
        >
          <LayoutGrid size={14} />
        </button>
        <button
          type="button"
          class:active={viewMode === 'list'}
          onclick={() => (viewMode = 'list')}
          title="Список"
          aria-label="Список"
        >
          <List size={14} />
        </button>
      </div>
      <button
        type="button"
        class="filter-chip"
        disabled={loading}
        onclick={() => load(true)}
        title="Обновить медиатеку"
      >
        <RefreshCw size={12} class={loading ? 'spin' : ''} />
      </button>
    </div>
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

  {#if section !== 'localFiles' && authStatuses.length > 0}
    {@const anyConnected = authStatuses.some((a) => enabledSources.includes(a.source) && a.connected)}
    {@const filterAuth = sourceFilter ? authStatuses.find((a) => a.source === sourceFilter) : null}
    {#if !anyConnected || (filterAuth && !filterAuth.connected)}
      <div class="lib-unauth-banner">
        <div class="lib-unauth-text">
          <strong>Вы не авторизованы</strong>
          <span>
            {#if filterAuth && !filterAuth.connected}
              Подключите {sourceFilter === 'yandex' ? 'Яндекс Музыку' : 'SoundCloud'}, чтобы видеть медиатеку этого сервиса.
            {:else}
              Войдите хотя бы в один музыкальный сервис, чтобы загрузить вашу медиатеку.
            {/if}
          </span>
        </div>
        {#if onOpenAuth}
          <button type="button" class="lib-unauth-btn" onclick={onOpenAuth}>Войти</button>
        {/if}
      </div>
    {/if}
  {/if}

  {#if loading}
    {#if viewMode === 'grid'}
      <div class="lib-bento-grid">
        {#each Array(9) as _, i (i)}
          <div class="skeleton-tile" class:featured={i === 0}></div>
        {/each}
      </div>
    {:else}
      <div class="tracks-table">
        {#each Array(8) as _, i (i)}
          <div class="skeleton-row"></div>
        {/each}
      </div>
    {/if}
  {:else if tracks.length === 0}
    <div class="state-msg">
      {section === 'localFiles'
        ? 'Локальные треки не найдены — добавьте папку с музыкой выше.'
        : 'Треки не найдены. Подключите сервисы в разделе аккаунтов.'}
    </div>
  {:else if section === 'albums' && !selectedAlbum}
    {#if viewMode === 'grid'}
      <div class="cards-grid">
        {#each groupedAlbums as alb (alb.title)}
          <button type="button" class="lib-card" onclick={() => (selectedAlbum = alb.title)}>
            <img src={alb.artwork} alt={alb.title} loading="lazy" />
            <div class="lc-title">{alb.title}</div>
            <div class="lc-sub">{alb.artist} · {alb.tracks.length} трек.</div>
          </button>
        {/each}
      </div>
    {:else}
      <div class="tracks-table">
        {#each groupedAlbums as alb, idx (alb.title)}
          <div
            class="track-row"
            role="button"
            tabindex="0"
            onclick={() => (selectedAlbum = alb.title)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                selectedAlbum = alb.title;
              }
            }}
          >
            <span class="idx">{idx + 1}</span>
            <span class="play-row-btn">
              <img src={alb.artwork} alt={alb.title} />
            </span>
            <div class="main-col">
              <span class="title-btn">{alb.title}</span>
              <span class="artist-btn">{alb.artist}</span>
            </div>
            <div class="album-col">{alb.tracks.length} трек.</div>
            <div class="source-col"><SourceBadge source={alb.source} /></div>
          </div>
        {/each}
      </div>
    {/if}
  {:else if section === 'artists'}
    {#if viewMode === 'grid'}
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
      <div class="tracks-table">
        {#each groupedArtists as art, idx (art.name)}
          <div
            class="track-row"
            role="button"
            tabindex="0"
            onclick={() => onOpenArtist?.(art.name)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onOpenArtist?.(art.name);
              }
            }}
          >
            <span class="idx">{idx + 1}</span>
            <span class="play-row-btn">
              <img src={art.artwork} alt={art.name} style="border-radius: 50%;" />
            </span>
            <div class="main-col">
              <span class="title-btn">{art.name}</span>
              <span class="artist-btn">Исполнитель</span>
            </div>
            <div class="album-col">{art.tracks.length} трек.</div>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    {#if section === 'albums' && selectedAlbum}
      <div class="album-bar">
        <button type="button" class="filter-chip" onclick={() => (selectedAlbum = null)}>
          ← Все альбомы
        </button>
        <span class="album-bar-title">{selectedAlbum}</span>
      </div>
    {/if}
    {#if viewMode === 'grid'}
      <div class="lib-bento-grid">
        {#each renderedTracks as track, idx (track.id)}
          {@const active = currentTrackId === track.id}
          {@const isLiked = likedIds.has(track.id)}
          <div
            class="bento-tile"
            class:featured={idx === 0 && renderedTracks.length > 4}
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
            <img src={track.artwork} alt={track.title} loading="lazy" />
            <div class="bento-grad"></div>
            <div class="bento-top-badges">
              <SourceBadge source={track.source} showLabel={false} />
              {#if onToggleLike}
                <button
                  type="button"
                  class="bento-like"
                  class:liked={isLiked}
                  onclick={(e) => {
                    e.stopPropagation();
                    onToggleLike(track);
                  }}
                  aria-label="Нравится"
                >
                  <Heart size={13} fill={isLiked ? 'currentColor' : 'none'} />
                </button>
              {/if}
            </div>
            <div class="bento-play-pill"><Play size={16} fill="currentColor" /></div>
            <div class="bento-meta">
              <div class="bento-title">{track.title}</div>
              <div class="bento-sub">{track.artist}</div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="tracks-table">
        {#each renderedTracks as track, idx (track.id)}
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

    {#if displayedTracks.length > visibleLimit}
      <div class="load-more-wrap">
        <button type="button" class="load-more-btn" onclick={() => (visibleLimit += 120)}>
          Показать ещё ({displayedTracks.length - visibleLimit} осталось)
        </button>
      </div>
    {/if}
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
    grid-template-columns: 28px 44px 1.6fr 1.1fr 140px 56px 64px;
    align-items: center;
    gap: 12px;
    padding: clamp(8px, 0.7vw, 11px) 12px;
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
    grid-template-columns: repeat(auto-fill, minmax(clamp(160px, 13vw, 204px), 1fr));
    gap: clamp(14px, 1.3vw, 20px);
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

  .lib-unauth-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--accent, #6366f1) 11%, var(--bg-surface));
    border: 1px solid color-mix(in srgb, var(--accent, #6366f1) 30%, transparent);
  }

  .lib-unauth-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .lib-unauth-text strong {
    font-size: 13px;
    color: var(--text-primary);
  }

  .lib-unauth-btn {
    flex-shrink: 0;
    padding: 7px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent, #6366f1);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .lib-unauth-btn:hover {
    filter: brightness(1.08);
  }

  .lib-view-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .view-toggle {
    display: inline-flex;
    padding: 3px;
    border-radius: 10px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    gap: 2px;
  }

  .view-toggle button {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    padding: 5px 8px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .view-toggle button.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .lib-bento-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    grid-auto-rows: 156px;
    grid-auto-flow: dense;
    gap: 12px;
  }

  .bento-tile {
    position: relative;
    border-radius: 14px;
    overflow: hidden;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: transform 140ms ease, border-color 140ms ease;
  }

  .bento-tile.featured {
    grid-column: span 2;
    grid-row: span 2;
  }

  .bento-tile:hover {
    transform: translateY(-2px);
    border-color: rgba(255, 255, 255, 0.22);
  }

  .bento-tile.active {
    border-color: var(--accent);
  }

  .bento-tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .bento-grad {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.12) 0%, rgba(0, 0, 0, 0.15) 45%, rgba(8, 8, 12, 0.88) 100%);
  }

  .bento-top-badges {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .bento-like {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: none;
    background: rgba(12, 12, 16, 0.7);
    color: rgba(255, 255, 255, 0.78);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .bento-like.liked {
    color: var(--accent);
  }

  .bento-play-pill {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%) scale(0.9);
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 140ms ease, transform 140ms ease;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  .bento-tile:hover .bento-play-pill {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }

  .bento-meta {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 9px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .bento-title {
    font-size: 12.5px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bento-tile.featured .bento-title {
    font-size: 15px;
  }

  .bento-sub {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.72);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .load-more-wrap {
    display: flex;
    justify-content: center;
    padding: 10px 0 6px;
  }

  .load-more-btn {
    padding: 8px 18px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .load-more-btn:hover {
    background: var(--bg-elevated);
  }

  .skeleton-tile {
    border-radius: 14px;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.04) 25%, rgba(255, 255, 255, 0.08) 50%, rgba(255, 255, 255, 0.04) 75%);
    background-size: 200% 100%;
    animation: skelShimmer 1.4s infinite linear;
  }

  .skeleton-tile.featured {
    grid-column: span 2;
    grid-row: span 2;
  }

  .skeleton-row {
    height: 48px;
    border-radius: 10px;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.03) 25%, rgba(255, 255, 255, 0.07) 50%, rgba(255, 255, 255, 0.03) 75%);
    background-size: 200% 100%;
    animation: skelShimmer 1.4s infinite linear;
  }

  @keyframes skelShimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>

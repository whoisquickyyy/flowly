<script lang="ts">
  import { Search, Play, ExternalLink, Sparkles, LayoutGrid, List, Heart } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import { fuzzySearchTracks, hasCyrillic, transliterateRuToEn } from '../fuzzySearch';
  import type { AuthStatus, GroupedSearchResult, SourceKind, Track } from '../types';

  interface Props {
    initialQuery?: string;
    enabledSources?: SourceKind[];
    authStatuses?: AuthStatus[];
    likedIds?: Set<string>;
    localLibrary?: Track[];
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onToggleLike?: (track: Track) => void;
    onOpenArtist?: (artist: string) => void;
    onOpenAuth?: () => void;
  }

  let {
    initialQuery = '',
    enabledSources = ['yandex', 'soundcloud'],
    authStatuses = [],
    likedIds = new Set(),
    localLibrary = [],
    onPlayTrack,
    onToggleLike,
    onOpenArtist,
    onOpenAuth,
  }: Props = $props();

  let query = $state('');
  let sourceFilter = $state<SourceKind | null>(null);
  let viewMode = $state<'grid' | 'list'>('grid');
  let groups = $state<GroupedSearchResult[]>([]);
  let loading = $state(false);

  const mergedTracks = $derived.by(() => {
    const out: Track[] = [];
    const seen = new Set<string>();
    const activeGroups = sourceFilter ? groups.filter((g) => g.source === sourceFilter) : groups;
    const maxLen = activeGroups.reduce((m, g) => Math.max(m, g.tracks.length), 0);
    for (let i = 0; i < maxLen; i++) {
      for (const g of activeGroups) {
        const t = g.tracks[i];
        if (t && !seen.has(t.id)) {
          seen.add(t.id);
          out.push(t);
        }
      }
    }
    return out;
  });

  const foundArtists = $derived.by(() => {
    const map = new Map<string, { name: string; artwork: string }>();
    for (const t of [...mergedTracks, ...localFuzzyMatches]) {
      const name = t.artist.split(/[,&]/)[0]?.trim() || t.artist;
      if (name && !map.has(name)) {
        map.set(name, { name, artwork: t.artwork });
        if (map.size >= 12) break;
      }
    }
    return Array.from(map.values());
  });

  const localFuzzyMatches = $derived.by(() => {
    if (!query.trim() || localLibrary.length === 0) return [];
    return fuzzySearchTracks(query, localLibrary, 8).filter((t) => enabledSources.includes(t.source));
  });

  $effect(() => {
    if (initialQuery) {
      query = initialQuery;
    }
  });

  async function runSearch() {
    loading = true;
    try {
      const effectiveFilter =
        sourceFilter && enabledSources.includes(sourceFilter)
          ? sourceFilter
          : enabledSources.length === 1
            ? enabledSources[0]
            : null;
      const res = await api.searchGrouped(query, effectiveFilter);
      const filtered = res.filter((g) => enabledSources.includes(g.source));

      // Если запрос на кириллице — дополнительно ищем по транслитерации (напр. «дефт панк» -> «daft punk»)
      const cleanQ = query.trim();
      if (cleanQ && hasCyrillic(cleanQ)) {
        const enQ = transliterateRuToEn(cleanQ);
        if (enQ && enQ !== cleanQ.toLowerCase()) {
          const enRes = await api.searchGrouped(enQ, effectiveFilter).catch(() => []);
          for (const g of filtered) {
            const matchG = enRes.find((eg) => eg.source === g.source);
            if (matchG) {
              const seen = new Set(g.tracks.map((t) => t.id));
              for (const t of matchG.tracks) {
                if (!seen.has(t.id)) {
                  g.tracks.push(t);
                  seen.add(t.id);
                }
              }
            }
          }
        }
      }

      groups = filtered;
    } finally {
      loading = false;
    }
  }

  let searchDebounce: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const q = query;
    if (searchDebounce) clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => {
      void q;
      runSearch();
    }, 260);
    return () => {
      if (searchDebounce) clearTimeout(searchDebounce);
    };
  });

  function formatDuration(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<section class="search-view">
  <form
    class="search-bar"
    onsubmit={(e) => {
      e.preventDefault();
      runSearch();
    }}
  >
    <Search size={16} />
    <input
      type="text"
      bind:value={query}
      placeholder="Поиск по Яндекс Музыке и SoundCloud..."
    />
    <button type="submit" class="search-btn">Найти</button>
  </form>

  <div class="search-toolbar">
    {#if enabledSources.length > 1}
      <div class="filters">
        <button class:active={sourceFilter === null} onclick={() => { sourceFilter = null; runSearch(); }}>Все</button>
        {#if enabledSources.includes('yandex')}
          <button class:active={sourceFilter === 'yandex'} onclick={() => { sourceFilter = 'yandex'; runSearch(); }}>Яндекс Музыка</button>
        {/if}
        {#if enabledSources.includes('soundcloud')}
          <button class:active={sourceFilter === 'soundcloud'} onclick={() => { sourceFilter = 'soundcloud'; runSearch(); }}>SoundCloud</button>
        {/if}
      </div>
    {/if}

    <div class="view-toggle">
      <button
        type="button"
        class:active={viewMode === 'grid'}
        onclick={() => (viewMode = 'grid')}
        title="Мозаика обложек"
        aria-label="Сетка"
      >
        <LayoutGrid size={14} />
      </button>
      <button
        type="button"
        class:active={viewMode === 'list'}
        onclick={() => (viewMode = 'list')}
        title="Список по сервисам"
        aria-label="Список"
      >
        <List size={14} />
      </button>
    </div>
  </div>

  {#if foundArtists.length > 0 && !loading}
    <div class="artists-strip">
      {#each foundArtists as art (art.name)}
        <button type="button" class="artist-pill" onclick={() => onOpenArtist?.(art.name)}>
          <img src={art.artwork} alt={art.name} />
          <span>{art.name}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if localFuzzyMatches.length > 0}
    <div class="source-group fuzzy-local">
      <div class="group-head">
        <span class="fuzzy-badge"><Sparkles size={12} /> Умный поиск в вашей медиатеке</span>
        <span>{localFuzzyMatches.length} совпад.</span>
      </div>
      <div class="group-tracks">
        {#each localFuzzyMatches as track (track.id)}
          <div class="s-row">
            <button class="s-cover" onclick={() => onPlayTrack(track, localFuzzyMatches)} aria-label="Слушать">
              <img src={track.artwork} alt={track.title} />
              <span class="s-play"><Play size={13} fill="currentColor" /></span>
            </button>
            <button class="s-meta" onclick={() => onPlayTrack(track, localFuzzyMatches)}>
              <div class="s-title">{track.title}</div>
              <div class="s-sub">{track.artist} · {track.album}</div>
            </button>
            <SourceBadge source={track.source} showLabel={false} />
            <span class="s-dur">{formatDuration(track.duration)}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if loading}
    {#if viewMode === 'grid'}
      <div class="search-bento-grid">
        {#each Array(9) as _, i (i)}
          <div class="s-skel-tile" class:featured={i === 0}></div>
        {/each}
      </div>
    {:else}
      <div class="groups-list">
        {#each Array(6) as _, i (i)}
          <div class="s-skel-row"></div>
        {/each}
      </div>
    {/if}
  {:else if viewMode === 'grid'}
    {#each groups as group (group.source)}
      {@const isConnected = authStatuses.find((a) => a.source === group.source)?.connected ?? false}
      {#if (!isConnected && authStatuses.length > 0) || group.error === 'Вы не авторизованы'}
        <div class="unauth-card">
          <div class="unauth-text">
            <strong>Вы не авторизованы ({group.sourceName})</strong>
            <span>Войдите в {group.sourceName}, чтобы искать и слушать треки из этого сервиса.</span>
          </div>
          {#if onOpenAuth}
            <button type="button" class="unauth-btn" onclick={onOpenAuth}>Войти</button>
          {/if}
        </div>
      {/if}
    {/each}

    {#if mergedTracks.length > 0}
      <div class="search-bento-grid">
        {#each mergedTracks as track, idx (track.id)}
          {@const isLiked = likedIds.has(track.id)}
          <div
            class="s-bento-tile"
            class:featured={idx === 0 && mergedTracks.length > 3}
            role="button"
            tabindex="0"
            onclick={() => onPlayTrack(track, mergedTracks)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onPlayTrack(track, mergedTracks);
              }
            }}
          >
            <img src={track.artwork} alt={track.title} loading="lazy" />
            <div class="s-bento-grad"></div>
            <div class="s-bento-top">
              <SourceBadge source={track.source} showLabel={false} />
              {#if onToggleLike}
                <button
                  type="button"
                  class="s-bento-like"
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
            <div class="s-bento-play"><Play size={17} fill="currentColor" /></div>
            <div class="s-bento-meta">
              <div class="s-bento-title">{track.title}</div>
              <div class="s-bento-sub">{track.artist}</div>
            </div>
          </div>
        {/each}
      </div>
    {:else if query.trim()}
      <div class="hint">Ничего не найдено по запросу «{query}».</div>
    {/if}
  {:else}
    <div class="groups-list">
      {#each groups as group (group.source)}
        {@const isConnected = authStatuses.find((a) => a.source === group.source)?.connected ?? false}
        <div class="source-group">
          <div class="group-head">
            <SourceBadge source={group.source} />
            <span class="group-count">{group.tracks.length}</span>
            {#if group.error && group.error !== 'Вы не авторизованы'}
              <span class="group-note">{group.error}</span>
            {/if}
          </div>

          {#if (!isConnected && authStatuses.length > 0) || group.error === 'Вы не авторизованы'}
            <div class="unauth-card">
              <div class="unauth-text">
                <strong>Вы не авторизованы</strong>
                <span>Войдите в {group.sourceName}, чтобы искать и слушать треки из этого сервиса.</span>
              </div>
              {#if onOpenAuth}
                <button type="button" class="unauth-btn" onclick={onOpenAuth}>Войти</button>
              {/if}
            </div>
          {:else if group.tracks.length > 0}
            <div class="group-tracks">
              {#each group.tracks as track (track.id)}
                <div class="s-row">
                  <button class="s-cover" onclick={() => onPlayTrack(track, group.tracks)}>
                    <img src={track.artwork} alt={track.title} />
                    <span class="s-play"><Play size={14} fill="currentColor" /></span>
                  </button>
                  <button type="button" class="s-meta" onclick={() => onPlayTrack(track, group.tracks)}>
                    <div class="s-title">{track.title}</div>
                    <div class="s-sub">{track.artist} · {track.album}</div>
                  </button>
                  <span class="s-dur">{formatDuration(track.duration)}</span>
                  <button
                    class="s-ext"
                    title="Открыть в оригинальном сервисе"
                    onclick={() => api.openExternalUrl(track.externalUrl)}
                  >
                    <ExternalLink size={14} />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .search-view {
    padding: 20px 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
  }

  .search-bar input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 14px;
    outline: none;
  }

  .search-btn {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #111215;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .filters {
    display: flex;
    gap: 6px;
  }

  .filters button {
    padding: 5px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
  }

  .filters button.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .groups-list {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .source-group {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .group-count {
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-elevated);
    padding: 1px 7px;
    border-radius: 999px;
  }

  .group-note {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-muted);
  }

  .unauth-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent, #6366f1) 10%, var(--bg-elevated));
    border: 1px solid color-mix(in srgb, var(--accent, #6366f1) 28%, transparent);
  }

  .unauth-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .unauth-text strong {
    font-size: 13px;
    color: var(--text-primary);
  }

  .unauth-btn {
    flex-shrink: 0;
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent, #6366f1);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .unauth-btn:hover {
    filter: brightness(1.08);
  }

  .group-tracks {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .s-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
  }

  .s-row:hover {
    background: var(--bg-elevated);
  }

  .s-cover {
    position: relative;
    width: 38px;
    height: 38px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: none;
    padding: 0;
    cursor: pointer;
    flex-shrink: 0;
  }

  .s-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .s-play {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    color: white;
    display: grid;
    place-items: center;
    opacity: 0;
  }

  .s-row:hover .s-play {
    opacity: 1;
  }

  .s-meta {
    flex: 1;
    min-width: 0;
    cursor: pointer;
    border: none;
    background: transparent;
    text-align: left;
    color: inherit;
    padding: 0;
  }

  .s-title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-sub {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-dur {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .s-ext {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 6px;
    border-radius: var(--radius-sm);
  }

  .s-ext:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .hint {
    padding: 32px;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .fuzzy-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: var(--accent);
  }

  .search-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .view-toggle {
    margin-left: auto;
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

  .artists-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .artist-pill {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px 4px 4px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 140ms ease, border-color 140ms ease;
  }

  .artist-pill:hover {
    background: var(--bg-elevated);
    border-color: rgba(255, 255, 255, 0.22);
  }

  .artist-pill img {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    object-fit: cover;
  }

  .search-bento-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    grid-auto-rows: 156px;
    grid-auto-flow: dense;
    gap: 12px;
  }

  .s-bento-tile {
    position: relative;
    border-radius: 14px;
    overflow: hidden;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: transform 140ms ease, border-color 140ms ease;
  }

  .s-bento-tile.featured {
    grid-column: span 2;
    grid-row: span 2;
  }

  .s-bento-tile:hover {
    transform: translateY(-2px);
    border-color: rgba(255, 255, 255, 0.22);
  }

  .s-bento-tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .s-bento-grad {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.1) 0%, rgba(0, 0, 0, 0.14) 45%, rgba(8, 8, 12, 0.88) 100%);
  }

  .s-bento-top {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .s-bento-like {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: none;
    background: rgba(12, 12, 16, 0.72);
    color: rgba(255, 255, 255, 0.78);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .s-bento-like.liked {
    color: var(--accent);
  }

  .s-bento-play {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%) scale(0.9);
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 140ms ease, transform 140ms ease;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  .s-bento-tile:hover .s-bento-play {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }

  .s-bento-meta {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 9px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .s-bento-title {
    font-size: 12.5px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-bento-tile.featured .s-bento-title {
    font-size: 16px;
  }

  .s-bento-sub {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.74);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .s-skel-tile {
    border-radius: 14px;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.04) 25%, rgba(255, 255, 255, 0.08) 50%, rgba(255, 255, 255, 0.04) 75%);
    background-size: 200% 100%;
    animation: sSkel 1.4s infinite linear;
  }

  .s-skel-tile.featured {
    grid-column: span 2;
    grid-row: span 2;
  }

  .s-skel-row {
    height: 46px;
    border-radius: 10px;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.03) 25%, rgba(255, 255, 255, 0.07) 50%, rgba(255, 255, 255, 0.03) 75%);
    background-size: 200% 100%;
    animation: sSkel 1.4s infinite linear;
  }

  @keyframes sSkel {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>

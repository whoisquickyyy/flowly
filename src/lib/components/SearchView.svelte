<script lang="ts">
  import { Search, Play, ExternalLink, Sparkles } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import { fuzzySearchTracks, hasCyrillic, transliterateRuToEn } from '../fuzzySearch';
  import type { GroupedSearchResult, SourceKind, Track } from '../types';

  interface Props {
    initialQuery?: string;
    enabledSources?: SourceKind[];
    localLibrary?: Track[];
    onPlayTrack: (track: Track, queue: Track[]) => void;
  }

  let {
    initialQuery = '',
    enabledSources = ['yandex', 'spotify', 'soundcloud'],
    localLibrary = [],
    onPlayTrack,
  }: Props = $props();

  let query = $state('');
  let sourceFilter = $state<SourceKind | null>(null);
  let groups = $state<GroupedSearchResult[]>([]);
  let loading = $state(false);

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
      placeholder="Поиск по Yandex Music, Spotify и SoundCloud..."
    />
    <button type="submit" class="search-btn">Найти</button>
  </form>

  {#if enabledSources.length > 1}
    <div class="filters">
      <button class:active={sourceFilter === null} onclick={() => { sourceFilter = null; runSearch(); }}>Все</button>
      {#if enabledSources.includes('yandex')}
        <button class:active={sourceFilter === 'yandex'} onclick={() => { sourceFilter = 'yandex'; runSearch(); }}>Яндекс Музыка</button>
      {/if}
      {#if enabledSources.includes('spotify')}
        <button class:active={sourceFilter === 'spotify'} onclick={() => { sourceFilter = 'spotify'; runSearch(); }}>Spotify</button>
      {/if}
      {#if enabledSources.includes('soundcloud')}
        <button class:active={sourceFilter === 'soundcloud'} onclick={() => { sourceFilter = 'soundcloud'; runSearch(); }}>SoundCloud</button>
      {/if}
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
    <div class="hint">Поиск по сервисам...</div>
  {:else}
    <div class="groups-list">
      {#each groups as group (group.source)}
        <div class="source-group">
          <div class="group-head">
            <SourceBadge source={group.source} />
            <span class="group-count">{group.tracks.length}</span>
            {#if group.error}
              <span class="group-note">{group.error}</span>
            {/if}
          </div>

          {#if group.tracks.length > 0}
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
</style>

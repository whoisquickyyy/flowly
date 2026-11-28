<script lang="ts">
  import { Search, Play, Sparkles, Heart, Clock, X } from 'lucide-svelte';
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
  let groups = $state<GroupedSearchResult[]>([]);
  let loading = $state(false);

  const RECENT_KEY = 'flowly_recent_searches';
  let recentQueries = $state<string[]>(
    (() => {
      try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(RECENT_KEY) : null;
        if (raw) {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed)) return parsed.filter((x): x is string => typeof x === 'string').slice(0, 10);
        }
      } catch {
        // ignore
      }
      return [];
    })()
  );

  function saveRecentQuery(rawQ: string) {
    const clean = rawQ.trim();
    if (clean.length < 2) return;
    const next = [clean, ...recentQueries.filter((x) => x.toLowerCase() !== clean.toLowerCase())].slice(0, 10);
    recentQueries = next;
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    } catch {
      // ignore
    }
  }

  function removeRecentQuery(target: string) {
    recentQueries = recentQueries.filter((x) => x !== target);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recentQueries));
    } catch {
      // ignore
    }
  }

  function clearRecentQueries() {
    recentQueries = [];
    try {
      localStorage.removeItem(RECENT_KEY);
    } catch {
      // ignore
    }
  }

  const MOOD_CARDS = [
    { title: 'Phonk', sub: 'Драйв и басы', q: 'phonk', grad: 'linear-gradient(135deg, #4c1d95, #be185d)' },
    { title: 'Lo-Fi & Chill', sub: 'Спокойный фон', q: 'lofi chill beats', grad: 'linear-gradient(135deg, #0f766e, #1d4ed8)' },
    { title: 'Русский рэп', sub: 'Новинки и хиты', q: 'русский рэп', grad: 'linear-gradient(135deg, #991b1b, #d97706)' },
    { title: 'Электроника', sub: 'Synth, House, Techno', q: 'electronic house', grad: 'linear-gradient(135deg, #0369a1, #6d28d9)' },
    { title: 'Инди и рок', sub: 'Гитары и атмосфера', q: 'indie rock', grad: 'linear-gradient(135deg, #3f6212, #0d9488)' },
    { title: 'Ночной драйв', sub: 'Для поздних поездок', q: 'night drive synthwave', grad: 'linear-gradient(135deg, #312e81, #701a75)' },
    { title: 'Для фокуса', sub: 'Кодинг и работа', q: 'ambient instrumental', grad: 'linear-gradient(135deg, #1e293b, #0f766e)' },
    { title: 'Саундтреки', sub: 'Кино и игры', q: 'soundtrack ost', grad: 'linear-gradient(135deg, #7c2d12, #9f1239)' },
  ];

  const libraryArtists = $derived.by(() => {
    const map = new Map<string, { name: string; artwork: string }>();
    for (const t of localLibrary) {
      const name = t.artist.split(/[,&]/)[0]?.trim() || t.artist;
      if (name && t.artwork && !map.has(name)) {
        map.set(name, { name, artwork: t.artwork });
        if (map.size >= 10) break;
      }
    }
    return Array.from(map.values());
  });

  const quickTracks = $derived(localLibrary.slice(0, 8));

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
      if (cleanQ && filtered.some((g) => g.tracks.length > 0)) {
        saveRecentQuery(cleanQ);
      }
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

<section class="search-view tab-enter">
  <form
    class="search-bar"
    onsubmit={(e) => {
      e.preventDefault();
      if (query.trim()) saveRecentQuery(query);
      runSearch();
    }}
  >
    <Search size={16} />
    <input
      type="text"
      bind:value={query}
      placeholder="Поиск по Яндекс Музыке и SoundCloud..."
    />
    {#if query}
      <button
        type="button"
        class="search-clear"
        onclick={() => {
          query = '';
          groups = [];
        }}
        aria-label="Очистить поиск"
      >
        <X size={14} />
      </button>
    {/if}
    <button type="submit" class="search-btn">Найти</button>
  </form>

  {#if enabledSources.length > 1}
    <div class="search-toolbar">
      <div class="filters">
        <button class:active={sourceFilter === null} onclick={() => { sourceFilter = null; runSearch(); }}>Все</button>
        {#if enabledSources.includes('yandex')}
          <button class:active={sourceFilter === 'yandex'} onclick={() => { sourceFilter = 'yandex'; runSearch(); }}>Яндекс Музыка</button>
        {/if}
        {#if enabledSources.includes('soundcloud')}
          <button class:active={sourceFilter === 'soundcloud'} onclick={() => { sourceFilter = 'soundcloud'; runSearch(); }}>SoundCloud</button>
        {/if}
      </div>
    </div>
  {/if}

  {#if !query.trim() && !loading}
    <div class="search-discover">
      {#if recentQueries.length > 0}
        <div class="disc-section">
          <div class="disc-head">
            <h3>Недавние запросы</h3>
            <button type="button" class="disc-clear-btn" onclick={clearRecentQueries}>Очистить</button>
          </div>
          <div class="recent-chips">
            {#each recentQueries as rq (rq)}
              <div class="recent-chip">
                <button
                  type="button"
                  class="recent-chip-main"
                  onclick={() => {
                    query = rq;
                    runSearch();
                  }}
                >
                  <Clock size={12} />
                  <span>{rq}</span>
                </button>
                <button
                  type="button"
                  class="recent-chip-del"
                  onclick={() => removeRecentQuery(rq)}
                  aria-label={`Удалить ${rq}`}
                >
                  <X size={11} />
                </button>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <div class="disc-section">
        <div class="disc-head">
          <h3>Обзор по жанрам и настроениям</h3>
        </div>
        <div class="mood-grid">
          {#each MOOD_CARDS as card (card.q)}
            <button
              type="button"
              class="mood-card"
              style="background: {card.grad}"
              onclick={() => {
                query = card.q;
                saveRecentQuery(card.q);
                runSearch();
              }}
            >
              <span class="mood-title">{card.title}</span>
              <span class="mood-sub">{card.sub}</span>
            </button>
          {/each}
        </div>
      </div>

      {#if libraryArtists.length > 0}
        <div class="disc-section">
          <div class="disc-head">
            <h3>Артисты из вашей медиатеки</h3>
          </div>
          <div class="fav-artists-grid">
            {#each libraryArtists as art (art.name)}
              <button type="button" class="fav-artist-card" onclick={() => onOpenArtist?.(art.name)}>
                <img src={art.artwork} alt={art.name} loading="lazy" />
                <span>{art.name}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if quickTracks.length > 0}
        <div class="disc-section">
          <div class="disc-head">
            <h3>Быстрый старт</h3>
          </div>
          <div class="quick-tracks-grid">
            {#each quickTracks as tr (tr.id)}
              <button
                type="button"
                class="quick-track-card"
                onclick={() => onPlayTrack(tr, quickTracks)}
              >
                <div class="qt-cover">
                  <img src={tr.artwork} alt={tr.title} loading="lazy" />
                  <span class="qt-play"><Play size={14} fill="currentColor" /></span>
                </div>
                <div class="qt-meta">
                  <div class="qt-title">{tr.title}</div>
                  <div class="qt-artist">{tr.artist}</div>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}

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
    <div class="search-bento-grid">
      {#each Array(9) as _, i (i)}
        <div class="s-skel-tile" class:featured={i === 0}></div>
      {/each}
    </div>
  {:else}
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
  {/if}
</section>

<style>
  .search-view {
    padding: clamp(20px, 2.2vw, 32px) clamp(24px, 3vw, 44px);
    max-width: 1260px;
    width: 100%;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
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

  .search-clear {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .search-clear:hover {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-primary);
  }

  .search-discover {
    display: flex;
    flex-direction: column;
    gap: 24px;
    margin-top: 4px;
  }

  .disc-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .disc-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .disc-head h3 {
    margin: 0;
    font-size: clamp(14px, 1.1vw, 16px);
    font-weight: 700;
    color: var(--text-primary);
  }

  .disc-clear-btn {
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .disc-clear-btn:hover {
    color: var(--text-primary);
  }

  .recent-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .recent-chip {
    display: inline-flex;
    align-items: center;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.055);
    border: 1px solid var(--border-subtle);
    overflow: hidden;
    transition: background 0.14s ease, border-color 0.14s ease;
  }

  .recent-chip:hover {
    background: rgba(255, 255, 255, 0.09);
    border-color: rgba(255, 255, 255, 0.18);
  }

  .recent-chip-main {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px 7px 12px;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 12.5px;
    cursor: pointer;
  }

  .recent-chip-del {
    border: none;
    background: transparent;
    color: var(--text-muted);
    padding: 7px 10px 7px 2px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .recent-chip-del:hover {
    color: #fb7185;
  }

  .mood-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(clamp(180px, 15vw, 240px), 1fr));
    gap: 12px;
  }

  .mood-card {
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 14px;
    padding: 16px 18px;
    min-height: 86px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-start;
    text-align: left;
    cursor: pointer;
    position: relative;
    overflow: hidden;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
    transition: transform 0.15s ease, filter 0.15s ease;
  }

  .mood-card:hover {
    transform: translateY(-2px);
    filter: brightness(1.1);
  }

  .mood-title {
    font-size: 15px;
    font-weight: 800;
    color: #fff;
  }

  .mood-sub {
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.8);
  }

  .fav-artists-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 12px;
  }

  .fav-artist-card {
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    border-radius: 14px;
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: background 0.14s ease, transform 0.14s ease;
  }

  .fav-artist-card:hover {
    background: rgba(255, 255, 255, 0.07);
    transform: translateY(-2px);
  }

  .fav-artist-card img {
    width: 68px;
    height: 68px;
    border-radius: 50%;
    object-fit: cover;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
  }

  .fav-artist-card span {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .quick-tracks-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 10px;
  }

  .quick-track-card {
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.035);
    border-radius: 12px;
    padding: 8px 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    cursor: pointer;
    transition: background 0.14s ease;
  }

  .quick-track-card:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .qt-cover {
    position: relative;
    width: 44px;
    height: 44px;
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
  }

  .qt-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .qt-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    opacity: 0;
    transition: opacity 0.12s ease;
  }

  .quick-track-card:hover .qt-play {
    opacity: 1;
  }

  .qt-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .qt-title {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .qt-artist {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  @keyframes sSkel {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>

<script lang="ts">
  import { ArrowLeft, Play, Sparkles, Heart, ExternalLink, Disc3, Headphones } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import type { ArtistYandexInfo, GroupedSearchResult, Track } from '../types';

  interface Props {
    artistName: string;
    currentTrackId: string | null;
    likedIds: Set<string>;
    onBack: () => void;
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onToggleLike: (track: Track) => void;
  }

  let {
    artistName,
    currentTrackId,
    likedIds,
    onBack,
    onPlayTrack,
    onToggleLike,
  }: Props = $props();

  let groups = $state<GroupedSearchResult[]>([]);
  let loading = $state(true);
  let yandexInfo = $state<ArtistYandexInfo | null>(null);
  let loadingRelease = $state(false);

  async function playLatestRelease(albumId: string) {
    if (loadingRelease) return;
    loadingRelease = true;
    try {
      const tracks = await api.getYandexAlbumTracks(albumId);
      if (tracks.length > 0) {
        onPlayTrack(tracks[0], tracks);
      }
    } catch (e) {
      console.error('Ошибка загрузки релиза:', e);
    } finally {
      loadingRelease = false;
    }
  }

  const allTracks = $derived(groups.flatMap((g) => g.tracks));
  const heroArt = $derived(yandexInfo?.artwork ?? allTracks[0]?.artwork ?? null);

  $effect(() => {
    const query = artistName.trim();
    if (!query) return;
    let cancelled = false;
    loading = true;
    yandexInfo = null;
    api
      .getArtistYandexInfo(query)
      .then((info) => {
        if (!cancelled) yandexInfo = info;
      })
      .catch(() => {});
    api
      .searchGrouped(query, null)
      .then((res) => {
        if (!cancelled) groups = res;
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  function formatListeners(count: number): string {
    return `${new Intl.NumberFormat('ru-RU').format(count)} слушателей за месяц`;
  }

  function formatReleaseType(t: string | null): string {
    switch (t?.toLowerCase()) {
      case 'single':
        return 'Сингл';
      case 'album':
        return 'Альбом';
      case 'ep':
        return 'EP';
      case 'podcast':
        return 'Подкаст';
      default:
        return 'Релиз';
    }
  }

  function formatDuration(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<section class="artist-view">
  <div class="artist-top">
    <button type="button" class="back-btn" onclick={onBack}>
      <ArrowLeft size={15} />
      <span>Назад</span>
    </button>
  </div>

  <div class="artist-hero">
    {#if heroArt}
      <img class="hero-art" src={heroArt} alt={artistName} />
    {:else}
      <div class="hero-art placeholder"><Disc3 size={34} /></div>
    {/if}

    <div class="hero-info">
      <span class="hero-tag">Артист</span>
      <h1>{artistName}</h1>
      {#if yandexInfo?.monthlyListeners != null}
        <div class="hero-listeners">
          <Headphones size={13} />
          <span>{formatListeners(yandexInfo.monthlyListeners)}</span>
        </div>
      {/if}
      <div class="hero-actions">
        <button
          type="button"
          class="btn-play-all"
          disabled={allTracks.length === 0}
          onclick={() => allTracks[0] && onPlayTrack(allTracks[0], allTracks)}
        >
          <Play size={14} fill="currentColor" />
          <span>Слушать всё</span>
        </button>
        <button
          type="button"
          class="btn-wave"
          disabled={allTracks.length === 0}
          onclick={() => {
            const shuffled = [...allTracks].sort(() => Math.random() - 0.5);
            if (shuffled[0]) onPlayTrack(shuffled[0], shuffled);
          }}
        >
          <Sparkles size={14} />
          <span>Микс по артисту</span>
        </button>
      </div>
    </div>
  </div>

  {#if yandexInfo?.latestRelease}
    {@const rel = yandexInfo.latestRelease}
    <div class="latest-release-card">
      <img class="rel-art" src={rel.artwork} alt={rel.title} />
      <div class="rel-info">
        <span class="rel-tag">Последний релиз · Яндекс Музыка</span>
        <div class="rel-title">{rel.title}</div>
        <div class="rel-meta">
          <span>{formatReleaseType(rel.releaseType)}</span>
          {#if rel.year}
            <span>· {rel.year}</span>
          {/if}
          {#if rel.trackCount}
            <span>· {rel.trackCount} трек.</span>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="btn-play-all"
        disabled={loadingRelease}
        onclick={() => playLatestRelease(rel.id)}
      >
        <Play size={14} fill="currentColor" />
        <span>{loadingRelease ? 'Загрузка...' : 'Слушать'}</span>
      </button>
      <button
        type="button"
        class="rel-open"
        onclick={() => api.openExternalUrl(rel.externalUrl)}
        title="Открыть релиз в Яндекс Музыке"
      >
        <ExternalLink size={14} />
        <span>Открыть</span>
      </button>
    </div>
  {/if}

  {#if loading}
    <div class="state-msg">Поиск треков артиста на всех площадках...</div>
  {:else if allTracks.length === 0}
    <div class="state-msg">Треки артиста не найдены.</div>
  {:else}
    <div class="groups-list">
      {#each groups as grp (grp.source)}
        {#if grp.tracks.length > 0}
          <div class="grp-block">
            <div class="grp-head">
              <SourceBadge source={grp.source} />
              <span class="grp-count">{grp.tracks.length} треков</span>
            </div>
            <div class="grp-tracks">
              {#each grp.tracks as tr (tr.id)}
                {@const active = currentTrackId === tr.id}
                {@const liked = likedIds.has(tr.id)}
                <div class="t-row" class:active>
                  <button type="button" class="t-main" onclick={() => onPlayTrack(tr, grp.tracks)}>
                    <img src={tr.artwork} alt={tr.title} />
                    <div class="t-meta">
                      <div class="t-title">{tr.title}</div>
                      <div class="t-sub">{tr.album} · {tr.artist}</div>
                    </div>
                  </button>
                  <span class="t-dur">{formatDuration(tr.duration)}</span>
                  <button
                    type="button"
                    class="t-act"
                    class:liked
                    onclick={() => onToggleLike(tr)}
                    aria-label="Лайк"
                  >
                    <Heart size={14} fill={liked ? 'currentColor' : 'none'} />
                  </button>
                  <button
                    type="button"
                    class="t-act"
                    onclick={() => api.openExternalUrl(tr.externalUrl)}
                    aria-label="Открыть в сервисе"
                  >
                    <ExternalLink size={14} />
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  .artist-view {
    padding: 18px 28px 28px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .back-btn:hover {
    background: var(--bg-elevated);
  }

  .artist-hero {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 18px;
    border-radius: 16px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
  }

  .hero-art {
    width: 112px;
    height: 112px;
    border-radius: 50%;
    object-fit: cover;
    border: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.55);
    flex-shrink: 0;
  }

  .hero-art.placeholder {
    display: grid;
    place-items: center;
    background: var(--bg-elevated);
    color: var(--text-muted);
  }

  .hero-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .hero-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent);
  }

  .hero-info h1 {
    font-size: clamp(22px, 2.5vw, 32px);
    font-weight: 800;
    letter-spacing: -0.02em;
  }

  .hero-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 2px;
  }

  .btn-play-all,
  .btn-wave {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .btn-play-all {
    border: none;
    background: var(--accent);
    color: #0b0b0e;
  }

  .btn-wave {
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .groups-list {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .grp-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .grp-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .grp-count {
    font-size: 11px;
    color: var(--text-muted);
  }

  .grp-tracks {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .t-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.02);
  }

  .t-row:hover {
    background: var(--bg-surface);
  }

  .t-row.active {
    background: var(--bg-elevated);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .t-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    padding: 0;
  }

  .t-main img {
    width: 38px;
    height: 38px;
    border-radius: 6px;
    object-fit: cover;
  }

  .t-meta {
    min-width: 0;
  }

  .t-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .t-sub {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .t-dur {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .t-act {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .t-act:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .t-act.liked {
    color: #ff3b5c;
  }

  .state-msg {
    padding: 32px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .hero-listeners {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .latest-release-card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    border-radius: 14px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
  }

  .rel-art {
    width: 60px;
    height: 60px;
    border-radius: 10px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.4);
  }

  .rel-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .rel-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .rel-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rel-meta {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .rel-open {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }

  .rel-open:hover {
    background: var(--bg-hover);
  }
</style>

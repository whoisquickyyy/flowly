<script lang="ts">
  import {
    ArrowLeft,
    Play,
    Sparkles,
    Heart,
    ExternalLink,
    Disc3,
    Headphones,
    X,
  } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import type { ArtistRelease, ArtistYandexInfo, GroupedSearchResult, Track } from '../types';

  interface Props {
    artistName: string;
    currentTrackId: string | null;
    likedIds: Set<string>;
    onBack: () => void;
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onToggleLike: (track: Track) => void;
    onOpenArtist?: (artist: string) => void;
  }

  let {
    artistName,
    currentTrackId,
    likedIds,
    onBack,
    onPlayTrack,
    onToggleLike,
    onOpenArtist = () => {},
  }: Props = $props();

  type ArtistTab = 'overview' | 'tracks' | 'albums' | 'singles' | 'similar';

  let groups = $state<GroupedSearchResult[]>([]);
  let loading = $state(true);
  let yandexInfo = $state<ArtistYandexInfo | null>(null);
  let activeTab = $state<ArtistTab>('overview');
  let loadingReleaseId = $state<string | null>(null);

  let openedAlbum = $state<ArtistRelease | null>(null);
  let openedAlbumTracks = $state<Track[]>([]);
  let openedAlbumLoading = $state(false);

  async function playRelease(rel: ArtistRelease) {
    if (loadingReleaseId === rel.id) return;
    loadingReleaseId = rel.id;
    try {
      const tracks = await api.getYandexAlbumTracks(rel.id);
      if (tracks.length > 0) {
        onPlayTrack(tracks[0], tracks);
      }
    } catch (e) {
      console.error('Ошибка загрузки релиза:', e);
    } finally {
      loadingReleaseId = null;
    }
  }

  async function openAlbumDrawer(rel: ArtistRelease) {
    openedAlbum = rel;
    openedAlbumTracks = [];
    openedAlbumLoading = true;
    try {
      openedAlbumTracks = await api.getYandexAlbumTracks(rel.id);
    } catch (e) {
      console.error('Ошибка загрузки альбома:', e);
    } finally {
      openedAlbumLoading = false;
    }
  }

  const allTracks = $derived(groups.flatMap((g) => g.tracks));
  const popularTracks = $derived(
    yandexInfo?.popularTracks && yandexInfo.popularTracks.length > 0
      ? yandexInfo.popularTracks
      : allTracks
  );
  const albums = $derived(yandexInfo?.albums ?? []);
  const singles = $derived(yandexInfo?.singles ?? []);
  const similarArtists = $derived(yandexInfo?.similarArtists ?? []);
  const genres = $derived(yandexInfo?.genres ?? []);
  const heroArt = $derived(yandexInfo?.artwork ?? allTracks[0]?.artwork ?? null);

  $effect(() => {
    const query = artistName.trim();
    if (!query) return;
    let cancelled = false;
    loading = true;
    yandexInfo = null;
    activeTab = 'overview';
    openedAlbum = null;
    openedAlbumTracks = [];

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

<section class="artist-view tab-enter">
  <div class="artist-top">
    <button type="button" class="back-btn" onclick={onBack}>
      <ArrowLeft size={15} />
      <span>Назад</span>
    </button>
  </div>

  <!-- Hero-баннер с размытым фоном и портретом -->
  <div class="artist-hero">
    {#if heroArt}
      <div class="hero-backdrop" style={`background-image: url('${heroArt}')`}></div>
    {/if}
    <div class="hero-scrim"></div>

    <div class="hero-content">
      {#if heroArt}
        <img class="hero-art" src={heroArt} alt={artistName} />
      {:else}
        <div class="hero-art placeholder"><Disc3 size={40} /></div>
      {/if}

      <div class="hero-info">
        <div class="hero-tags-row">
          <span class="hero-tag">Артист</span>
          {#each genres as g (g)}
            <span class="hero-genre">{g}</span>
          {/each}
        </div>
        <h1>{yandexInfo?.name || artistName}</h1>
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
            disabled={popularTracks.length === 0}
            onclick={() => popularTracks[0] && onPlayTrack(popularTracks[0], popularTracks)}
          >
            <Play size={14} fill="currentColor" />
            <span>Слушать</span>
          </button>
          <button
            type="button"
            class="btn-wave"
            disabled={popularTracks.length === 0}
            onclick={() => {
              const shuffled = [...popularTracks].sort(() => Math.random() - 0.5);
              if (shuffled[0]) onPlayTrack(shuffled[0], shuffled);
            }}
          >
            <Sparkles size={14} />
            <span>Микс по артисту</span>
          </button>
          {#if yandexInfo?.externalUrl}
            <button
              type="button"
              class="btn-ext"
              onclick={() => yandexInfo && api.openExternalUrl(yandexInfo.externalUrl)}
              title="Открыть в Яндекс Музыке"
            >
              <ExternalLink size={14} />
              <span>Яндекс Музыка</span>
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>

  <!-- Навигация по разделам карточки -->
  <div class="artist-tabs">
    <button
      type="button"
      class="a-tab"
      class:active={activeTab === 'overview'}
      onclick={() => (activeTab = 'overview')}
    >
      Обзор
    </button>
    <button
      type="button"
      class="a-tab"
      class:active={activeTab === 'tracks'}
      onclick={() => (activeTab = 'tracks')}
    >
      Треки ({popularTracks.length})
    </button>
    {#if albums.length > 0}
      <button
        type="button"
        class="a-tab"
        class:active={activeTab === 'albums'}
        onclick={() => (activeTab = 'albums')}
      >
        Альбомы ({albums.length})
      </button>
    {/if}
    {#if singles.length > 0}
      <button
        type="button"
        class="a-tab"
        class:active={activeTab === 'singles'}
        onclick={() => (activeTab = 'singles')}
      >
        Синглы и EP ({singles.length})
      </button>
    {/if}
    {#if similarArtists.length > 0}
      <button
        type="button"
        class="a-tab"
        class:active={activeTab === 'similar'}
        onclick={() => (activeTab = 'similar')}
      >
        Похожие ({similarArtists.length})
      </button>
    {/if}
  </div>

  <!-- Встроенный просмотрщик выбранного альбома/сингла -->
  {#if openedAlbum}
    <div class="album-drawer">
      <div class="album-drawer-head">
        <img class="drawer-art" src={openedAlbum.artwork} alt={openedAlbum.title} />
        <div class="drawer-meta">
          <span class="rel-tag">
            {formatReleaseType(openedAlbum.releaseType)}
            {#if openedAlbum.year} · {openedAlbum.year}{/if}
          </span>
          <h3>{openedAlbum.title}</h3>
          {#if openedAlbumTracks.length > 0}
            <div class="drawer-acts">
              <button
                type="button"
                class="btn-play-all"
                onclick={() => onPlayTrack(openedAlbumTracks[0], openedAlbumTracks)}
              >
                <Play size={13} fill="currentColor" />
                <span>Слушать релиз</span>
              </button>
              <button
                type="button"
                class="btn-ext"
                onclick={() => openedAlbum && api.openExternalUrl(openedAlbum.externalUrl)}
              >
                <ExternalLink size={13} />
                <span>Открыть</span>
              </button>
            </div>
          {/if}
        </div>
        <button
          type="button"
          class="drawer-close"
          onclick={() => (openedAlbum = null)}
          aria-label="Закрыть релиз"
        >
          <X size={16} />
        </button>
      </div>

      {#if openedAlbumLoading}
        <div class="state-msg">Загружаем треклист...</div>
      {:else if openedAlbumTracks.length === 0}
        <div class="state-msg">Треки релиза не найдены.</div>
      {:else}
        <div class="grp-tracks">
          {#each openedAlbumTracks as tr, i (tr.id)}
            {@const active = currentTrackId === tr.id}
            {@const liked = likedIds.has(tr.id)}
            <div class="t-row" class:active>
              <button
                type="button"
                class="t-main"
                onclick={() => onPlayTrack(tr, openedAlbumTracks)}
              >
                <span class="t-idx">{i + 1}</span>
                <div class="t-meta">
                  <div class="t-title">{tr.title}</div>
                  <div class="t-sub">{tr.artist}</div>
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
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  {#if activeTab === 'overview'}
    <!-- Последний релиз -->
    {#if yandexInfo?.latestRelease}
      {@const rel = yandexInfo.latestRelease}
      <div class="latest-release-card">
        <button type="button" class="rel-art-btn" onclick={() => openAlbumDrawer(rel)}>
          <img class="rel-art" src={rel.artwork} alt={rel.title} />
        </button>
        <div class="rel-info">
          <span class="rel-tag">Последний релиз · Яндекс Музыка</span>
          <button type="button" class="rel-title-btn" onclick={() => openAlbumDrawer(rel)}>
            {rel.title}
          </button>
          <div class="rel-meta">
            <span>{formatReleaseType(rel.releaseType)}</span>
            {#if rel.year}<span>· {rel.year}</span>{/if}
            {#if rel.trackCount}<span>· {rel.trackCount} трек.</span>{/if}
          </div>
        </div>
        <button
          type="button"
          class="btn-play-all"
          disabled={loadingReleaseId === rel.id}
          onclick={() => playRelease(rel)}
        >
          <Play size={14} fill="currentColor" />
          <span>{loadingReleaseId === rel.id ? 'Загрузка...' : 'Слушать'}</span>
        </button>
      </div>
    {/if}

    <!-- Популярные треки -->
    <div class="section-block">
      <div class="sec-head">
        <h2>Популярные треки</h2>
        {#if popularTracks.length > 6}
          <button type="button" class="sec-more" onclick={() => (activeTab = 'tracks')}>
            Смотреть все ({popularTracks.length})
          </button>
        {/if}
      </div>
      {#if loading && popularTracks.length === 0}
        <div class="state-msg">Загружаем треки...</div>
      {:else if popularTracks.length === 0}
        <div class="state-msg">Треки артиста не найдены.</div>
      {:else}
        <div class="grp-tracks">
          {#each popularTracks.slice(0, 6) as tr, idx (tr.id)}
            {@const active = currentTrackId === tr.id}
            {@const liked = likedIds.has(tr.id)}
            <div class="t-row" class:active>
              <button type="button" class="t-main" onclick={() => onPlayTrack(tr, popularTracks)}>
                <span class="t-idx">{idx + 1}</span>
                <img src={tr.artwork} alt={tr.title} />
                <div class="t-meta">
                  <div class="t-title">{tr.title}</div>
                  <div class="t-sub">{tr.album}</div>
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
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Альбомы -->
    {#if albums.length > 0}
      <div class="section-block">
        <div class="sec-head">
          <h2>Альбомы</h2>
          {#if albums.length > 6}
            <button type="button" class="sec-more" onclick={() => (activeTab = 'albums')}>
              Все ({albums.length})
            </button>
          {/if}
        </div>
        <div class="releases-grid">
          {#each albums.slice(0, 8) as alb (alb.id)}
            <div class="release-card">
              <div class="rc-cover-wrap">
                <button type="button" class="rc-open-area" onclick={() => openAlbumDrawer(alb)}>
                  <img src={alb.artwork} alt={alb.title} />
                </button>
                <button
                  type="button"
                  class="rc-play"
                  onclick={() => playRelease(alb)}
                  aria-label={`Слушать ${alb.title}`}
                >
                  <Play size={16} fill="currentColor" />
                </button>
              </div>
              <button type="button" class="rc-title" onclick={() => openAlbumDrawer(alb)}>
                {alb.title}
              </button>
              <div class="rc-sub">
                {alb.year ?? formatReleaseType(alb.releaseType)}
                {#if alb.trackCount} · {alb.trackCount} тр.{/if}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Синглы и EP -->
    {#if singles.length > 0}
      <div class="section-block">
        <div class="sec-head">
          <h2>Синглы и EP</h2>
          {#if singles.length > 6}
            <button type="button" class="sec-more" onclick={() => (activeTab = 'singles')}>
              Все ({singles.length})
            </button>
          {/if}
        </div>
        <div class="releases-grid">
          {#each singles.slice(0, 8) as sng (sng.id)}
            <div class="release-card">
              <div class="rc-cover-wrap">
                <button type="button" class="rc-open-area" onclick={() => openAlbumDrawer(sng)}>
                  <img src={sng.artwork} alt={sng.title} />
                </button>
                <button
                  type="button"
                  class="rc-play"
                  onclick={() => playRelease(sng)}
                  aria-label={`Слушать ${sng.title}`}
                >
                  <Play size={16} fill="currentColor" />
                </button>
              </div>
              <button type="button" class="rc-title" onclick={() => openAlbumDrawer(sng)}>
                {sng.title}
              </button>
              <div class="rc-sub">
                {sng.year ?? ''} · {formatReleaseType(sng.releaseType)}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Похожие артисты -->
    {#if similarArtists.length > 0}
      <div class="section-block">
        <div class="sec-head">
          <h2>Похожие артисты</h2>
        </div>
        <div class="similar-grid">
          {#each similarArtists as sim (sim.id)}
            <button
              type="button"
              class="similar-card"
              onclick={() => onOpenArtist(sim.name)}
            >
              {#if sim.artwork}
                <img src={sim.artwork} alt={sim.name} />
              {:else}
                <div class="sim-placeholder"><Disc3 size={28} /></div>
              {/if}
              <span>{sim.name}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {:else if activeTab === 'tracks'}
    <div class="groups-list">
      {#if popularTracks.length > 0}
        <div class="grp-block">
          <div class="grp-head">
            <span class="grp-label">Популярные треки</span>
            <span class="grp-count">{popularTracks.length} треков</span>
          </div>
          <div class="grp-tracks">
            {#each popularTracks as tr, idx (tr.id)}
              {@const active = currentTrackId === tr.id}
              {@const liked = likedIds.has(tr.id)}
              <div class="t-row" class:active>
                <button type="button" class="t-main" onclick={() => onPlayTrack(tr, popularTracks)}>
                  <span class="t-idx">{idx + 1}</span>
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
  {:else if activeTab === 'albums'}
    <div class="releases-grid full">
      {#each albums as alb (alb.id)}
        <div class="release-card">
          <div class="rc-cover-wrap">
            <button type="button" class="rc-open-area" onclick={() => openAlbumDrawer(alb)}>
              <img src={alb.artwork} alt={alb.title} />
            </button>
            <button
              type="button"
              class="rc-play"
              onclick={() => playRelease(alb)}
              aria-label={`Слушать ${alb.title}`}
            >
              <Play size={16} fill="currentColor" />
            </button>
          </div>
          <button type="button" class="rc-title" onclick={() => openAlbumDrawer(alb)}>
            {alb.title}
          </button>
          <div class="rc-sub">
            {alb.year ?? formatReleaseType(alb.releaseType)}
            {#if alb.trackCount} · {alb.trackCount} тр.{/if}
          </div>
        </div>
      {/each}
    </div>
  {:else if activeTab === 'singles'}
    <div class="releases-grid full">
      {#each singles as sng (sng.id)}
        <div class="release-card">
          <div class="rc-cover-wrap">
            <button type="button" class="rc-open-area" onclick={() => openAlbumDrawer(sng)}>
              <img src={sng.artwork} alt={sng.title} />
            </button>
            <button
              type="button"
              class="rc-play"
              onclick={() => playRelease(sng)}
              aria-label={`Слушать ${sng.title}`}
            >
              <Play size={16} fill="currentColor" />
            </button>
          </div>
          <button type="button" class="rc-title" onclick={() => openAlbumDrawer(sng)}>
            {sng.title}
          </button>
          <div class="rc-sub">
            {sng.year ?? ''} · {formatReleaseType(sng.releaseType)}
          </div>
        </div>
      {/each}
    </div>
  {:else if activeTab === 'similar'}
    <div class="similar-grid">
      {#each similarArtists as sim (sim.id)}
        <button
          type="button"
          class="similar-card"
          onclick={() => onOpenArtist(sim.name)}
        >
          {#if sim.artwork}
            <img src={sim.artwork} alt={sim.name} />
          {:else}
            <div class="sim-placeholder"><Disc3 size={28} /></div>
          {/if}
          <span>{sim.name}</span>
        </button>
      {/each}
    </div>
  {/if}
</section>

<style>
  .artist-view {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 20px 24px 120px;
  }

  .artist-top {
    display: flex;
    align-items: center;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .artist-hero {
    position: relative;
    overflow: hidden;
    border-radius: 22px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    min-height: 220px;
    display: flex;
    align-items: flex-end;
  }

  .hero-backdrop {
    position: absolute;
    inset: -24px;
    background-size: cover;
    background-position: center 25%;
    filter: blur(28px) saturate(1.25) brightness(0.65);
    transform: scale(1.08);
    pointer-events: none;
  }

  .hero-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      180deg,
      rgba(12, 12, 16, 0.28) 0%,
      rgba(12, 12, 16, 0.78) 65%,
      rgba(12, 12, 16, 0.94) 100%
    );
    pointer-events: none;
  }

  .hero-content {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: flex-end;
    gap: 22px;
    padding: 26px;
    width: 100%;
    flex-wrap: wrap;
  }

  .hero-art {
    width: 148px;
    height: 148px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
    border: 2px solid rgba(255, 255, 255, 0.16);
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.55);
  }

  .hero-art.placeholder {
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-secondary);
  }

  .hero-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }

  .hero-tags-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .hero-tag {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent);
  }

  .hero-genre {
    padding: 2px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-secondary);
    font-size: 10.5px;
    font-weight: 600;
    text-transform: capitalize;
  }

  .hero-info h1 {
    font-size: 34px;
    font-weight: 800;
    margin: 0;
    letter-spacing: -0.02em;
    line-height: 1.08;
    color: #fff;
  }

  .hero-listeners {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.72);
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
    flex-wrap: wrap;
  }

  .btn-play-all,
  .btn-wave,
  .btn-ext {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 9px 18px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
    border: none;
  }

  .btn-play-all {
    background: var(--accent);
    color: #0b0b0e;
  }

  .btn-wave,
  .btn-ext {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text-primary);
    border: 1px solid rgba(255, 255, 255, 0.14);
  }

  .artist-tabs {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .a-tab {
    padding: 7px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-secondary);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .a-tab.active {
    background: var(--accent);
    color: #0b0b0e;
    border-color: var(--accent);
  }

  .latest-release-card {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 18px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .rel-art-btn {
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
    flex-shrink: 0;
  }

  .rel-art {
    width: 64px;
    height: 64px;
    border-radius: 10px;
    object-fit: cover;
    display: block;
  }

  .rel-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .rel-tag {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .rel-title-btn {
    border: none;
    background: transparent;
    padding: 0;
    text-align: left;
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rel-meta {
    display: flex;
    gap: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .album-drawer {
    padding: 16px 18px;
    border-radius: 18px;
    background: var(--bg-surface);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .album-drawer-head {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .drawer-art {
    width: 84px;
    height: 84px;
    border-radius: 12px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .drawer-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .drawer-meta h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .drawer-acts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .drawer-close {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    align-self: flex-start;
  }

  .section-block {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .sec-head h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .sec-more {
    border: none;
    background: transparent;
    color: var(--accent);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .releases-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
    gap: 14px;
  }

  .release-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 10px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-subtle);
    transition: background 0.15s ease;
  }

  .release-card:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .rc-cover-wrap {
    position: relative;
    aspect-ratio: 1 / 1;
    border-radius: 10px;
    overflow: hidden;
  }

  .rc-open-area {
    border: none;
    padding: 0;
    width: 100%;
    height: 100%;
    background: transparent;
    cursor: pointer;
    display: block;
  }

  .rc-open-area img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .rc-play {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0;
    transform: translateY(4px);
    transition: opacity 0.15s ease, transform 0.15s ease;
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.45);
  }

  .release-card:hover .rc-play {
    opacity: 1;
    transform: translateY(0);
  }

  .rc-title {
    border: none;
    background: transparent;
    padding: 0;
    text-align: left;
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rc-sub {
    font-size: 11.5px;
    color: var(--text-muted);
  }

  .similar-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 14px;
  }

  .similar-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 12px 8px;
    border-radius: 14px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-primary);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    text-align: center;
  }

  .similar-card:hover {
    background: rgba(255, 255, 255, 0.07);
  }

  .similar-card img,
  .sim-placeholder {
    width: 82px;
    height: 82px;
    border-radius: 50%;
    object-fit: cover;
  }

  .sim-placeholder {
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-secondary);
  }

  .groups-list {
    display: flex;
    flex-direction: column;
    gap: 20px;
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

  .grp-label {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .grp-count {
    font-size: 12px;
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
    padding: 6px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid transparent;
  }

  .t-row:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .t-row.active {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
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

  .t-idx {
    width: 20px;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .t-main img {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .t-meta {
    min-width: 0;
    flex: 1;
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
    font-size: 11.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .t-dur {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .t-act {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 4px;
    display: grid;
    place-items: center;
  }

  .t-act.liked {
    color: var(--accent);
  }

  .state-msg {
    font-size: 13px;
    color: var(--text-muted);
    padding: 14px 0;
  }
</style>

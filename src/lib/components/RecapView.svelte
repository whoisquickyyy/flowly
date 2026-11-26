<script lang="ts">
  import { onMount } from 'svelte';
  import {
    BarChart3,
    Clock,
    Disc3,
    Heart,
    Music2,
    Play,
    RefreshCw,
    Sparkles,
    Users,
  } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import * as api from '../api';
  import type { ListeningRecap, RecapPeriod, Track } from '../types';

  interface Props {
    currentTrackId: string | null;
    likedIds: Set<string>;
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onToggleLike: (track: Track) => void;
    onOpenArtist: (artist: string) => void;
  }

  let { currentTrackId, likedIds, onPlayTrack, onToggleLike, onOpenArtist }: Props = $props();

  let period = $state<RecapPeriod>('day');
  let recap = $state<ListeningRecap | null>(null);
  let loading = $state(true);

  const periods: { id: RecapPeriod; label: string }[] = [
    { id: 'day', label: 'Сегодня' },
    { id: 'week', label: 'Неделя' },
    { id: 'month', label: 'Месяц' },
    { id: 'all', label: 'Всё время' },
  ];

  const genrePalette = ['#ff3b5c', '#f59e0b', '#10b981', '#3b82f6', '#a855f7', '#ec4899'];

  async function loadRecap(target: RecapPeriod = period) {
    loading = true;
    try {
      recap = await api.getListeningRecap(target);
    } catch (e) {
      console.error('Ошибка загрузки итогов:', e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadRecap(period);
  });

  function selectPeriod(next: RecapPeriod) {
    period = next;
    loadRecap(next);
  }

  function formatTotalTime(sec: number): string {
    if (sec < 60) return `${sec} сек`;
    const mins = Math.floor(sec / 60);
    const hrs = Math.floor(mins / 60);
    const remMins = mins % 60;
    if (hrs === 0) return `${mins} мин`;
    return `${hrs} ч ${remMins} мин`;
  }

  function playTopTracks() {
    if (!recap || recap.topTracks.length === 0) return;
    const list = recap.topTracks.map((t) => t.track);
    onPlayTrack(list[0], list);
  }
</script>

<section class="recap-view">
  <div class="recap-head">
    <div class="head-title">
      <BarChart3 size={20} />
      <div>
        <h2>Итоги прослушивания</h2>
        <p>Персональная статистика и вайб-разбор на основе твоей истории</p>
      </div>
    </div>

    <div class="head-controls">
      <div class="period-pills">
        {#each periods as p (p.id)}
          <button
            type="button"
            class="period-btn"
            class:active={period === p.id}
            onclick={() => selectPeriod(p.id)}
          >
            {p.label}
          </button>
        {/each}
      </div>
      <button
        type="button"
        class="refresh-btn"
        onclick={() => loadRecap(period)}
        title="Обновить итоги"
        aria-label="Обновить итоги"
      >
        <RefreshCw size={14} class={loading ? 'spin' : ''} />
      </button>
    </div>
  </div>

  {#if loading && !recap}
    <div class="empty-box">Собираем твои музыкальные итоги...</div>
  {:else if recap}
    <!-- Hero-карточка вайба -->
    <div
      class="vibe-hero"
      style={`--vibe-accent: ${recap.summary.accentColor || '#6366f1'}`}
    >
      <div class="vibe-top">
        <span class="vibe-badge">
          <Sparkles size={13} />
          <span>{recap.summary.badge}</span>
        </span>
        {#if recap.topTracks.length > 0}
          <button type="button" class="play-top-btn" onclick={playTopTracks}>
            <Play size={14} fill="currentColor" />
            <span>Запустить топ периода</span>
          </button>
        {/if}
      </div>

      <h3>{recap.summary.headline}</h3>
      <p>{recap.summary.description}</p>
    </div>

    <!-- 4 метрики -->
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-lbl"><Clock size={13} /> <span>Время в музыке</span></div>
        <div class="stat-val">{formatTotalTime(recap.totalSec)}</div>
      </div>
      <div class="stat-card">
        <div class="stat-lbl"><Disc3 size={13} /> <span>Прослушиваний</span></div>
        <div class="stat-val">
          {recap.totalPlays}
          <span class="stat-sub">({recap.uniqueTracks} уник.)</span>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-lbl"><Users size={13} /> <span>Артистов</span></div>
        <div class="stat-val">{recap.uniqueArtists}</div>
      </div>
      <div class="stat-card">
        <div class="stat-lbl"><Music2 size={13} /> <span>Главный жанр</span></div>
        <div class="stat-val genre-val">{recap.topGenre ?? '—'}</div>
      </div>
    </div>

    {#if recap.genres.length > 0}
      <div class="block-card">
        <div class="block-head">Распределение жанров</div>
        <div class="genre-bar">
          {#each recap.genres as g, i (g.genre)}
            <div
              class="genre-seg"
              style={`width: ${Math.max(g.percent, 4)}%; background: ${genrePalette[i % genrePalette.length]}`}
              title={`${g.genre}: ${g.percent}%`}
            ></div>
          {/each}
        </div>
        <div class="genre-tags">
          {#each recap.genres as g, i (g.genre)}
            <span class="g-tag">
              <span class="g-dot" style={`background: ${genrePalette[i % genrePalette.length]}`}></span>
              <span>{g.genre}</span>
              <span class="g-pct">{g.percent}%</span>
            </span>
          {/each}
        </div>
      </div>
    {/if}

    {#if recap.topArtists.length > 0}
      <div class="block-card">
        <div class="block-head">Топ артистов</div>
        <div class="artists-grid">
          {#each recap.topArtists as art, idx (art.name)}
            <button type="button" class="artist-card" onclick={() => onOpenArtist(art.name)}>
              <span class="rank-pill">#{idx + 1}</span>
              <img src={art.artwork} alt={art.name} />
              <div class="art-name">{art.name}</div>
              <div class="art-meta">{art.plays} просл. · {formatTotalTime(art.totalSec)}</div>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if recap.topTracks.length > 0}
      <div class="block-card">
        <div class="block-head">Топ треков</div>
        <div class="tracks-list">
          {#each recap.topTracks as item, idx (item.track.id)}
            {@const tr = item.track}
            {@const active = currentTrackId === tr.id}
            {@const liked = likedIds.has(tr.id)}
            <div class="t-row" class:active>
              <span class="t-rank">{idx + 1}</span>
              <button
                type="button"
                class="t-main"
                onclick={() => onPlayTrack(tr, recap?.topTracks.map((x) => x.track) ?? [tr])}
              >
                <img src={tr.artwork} alt={tr.title} />
                <div class="t-meta">
                  <div class="t-title">{tr.title}</div>
                  <div class="t-sub">{tr.artist}{tr.genre ? ` · ${tr.genre}` : ''}</div>
                </div>
              </button>
              <SourceBadge source={tr.source} showLabel={false} />
              <span class="t-plays">{item.plays}×</span>
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
      </div>
    {/if}
  {/if}
</section>

<style>
  .recap-view {
    padding: 20px 28px 34px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .recap-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
  }

  .head-title {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--accent);
  }

  .head-title h2 {
    font-size: 20px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .head-title p {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .head-controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .period-pills {
    display: inline-flex;
    padding: 3px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    gap: 2px;
  }

  .period-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    padding: 6px 12px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .period-btn.active {
    background: var(--accent);
    color: #0b0b0e;
  }

  .refresh-btn {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .refresh-btn:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  :global(.spin) {
    animation: recap-spin 0.9s linear infinite;
  }

  @keyframes recap-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .vibe-hero {
    padding: 20px 22px;
    border-radius: 18px;
    border: 1px solid color-mix(in srgb, var(--vibe-accent) 42%, transparent);
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--vibe-accent) 26%, rgba(18, 18, 24, 0.92)),
      rgba(16, 16, 22, 0.88)
    );
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.35);
  }

  .vibe-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }

  .vibe-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 700;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .play-top-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 999px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .vibe-hero h3 {
    font-size: clamp(20px, 2.2vw, 26px);
    font-weight: 800;
    letter-spacing: -0.02em;
  }

  .vibe-hero p {
    font-size: 13.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.84);
    max-width: 760px;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 12px;
  }

  .stat-card {
    padding: 14px 16px;
    border-radius: 14px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .stat-lbl {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .stat-val {
    font-size: 20px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .stat-sub {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .genre-val {
    text-transform: capitalize;
  }

  .block-card {
    padding: 16px 18px;
    border-radius: 16px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .block-head {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .genre-bar {
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    display: flex;
    gap: 2px;
    background: rgba(255, 255, 255, 0.06);
  }

  .genre-seg {
    height: 100%;
    border-radius: 4px;
  }

  .genre-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .g-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.05);
    font-size: 12px;
    text-transform: capitalize;
  }

  .g-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .g-pct {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .artists-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 12px;
  }

  .artist-card {
    position: relative;
    padding: 12px;
    border-radius: 14px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-primary);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    cursor: pointer;
    transition: background 0.15s ease, transform 0.15s ease;
  }

  .artist-card:hover {
    background: var(--bg-elevated);
    transform: translateY(-2px);
  }

  .rank-pill {
    position: absolute;
    top: 8px;
    left: 8px;
    font-size: 10px;
    font-weight: 800;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    color: var(--accent);
  }

  .artist-card img {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    object-fit: cover;
  }

  .art-name {
    font-size: 13px;
    font-weight: 700;
    width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .art-meta {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .tracks-list {
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
    border: 1px solid transparent;
  }

  .t-row:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .t-row.active {
    background: var(--bg-elevated);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .t-rank {
    width: 22px;
    text-align: center;
    font-size: 12px;
    font-weight: 700;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
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
  }

  .t-main img {
    width: 38px;
    height: 38px;
    border-radius: 7px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .t-meta {
    min-width: 0;
  }

  .t-title {
    font-size: 13px;
    font-weight: 600;
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

  .t-plays {
    font-size: 12px;
    font-weight: 700;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    min-width: 32px;
    text-align: right;
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

  .empty-box {
    padding: 40px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }
</style>

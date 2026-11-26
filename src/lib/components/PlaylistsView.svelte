<script lang="ts">
  import { Plus, Play, Trash2, ListMusic } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import type { Playlist, Track } from '../types';

  interface Props {
    playlists: Playlist[];
    onCreatePlaylist: (title: string) => Promise<void>;
    onPlayTrack: (track: Track, queue: Track[]) => void;
    onRemoveTrack: (playlistId: string, trackId: string) => Promise<void>;
  }

  let { playlists, onCreatePlaylist, onPlayTrack, onRemoveTrack }: Props = $props();

  let newTitle = $state('');
  let selectedId = $state<string | null>(null);

  $effect(() => {
    if (!selectedId && playlists.length > 0) {
      selectedId = playlists[0].id;
    }
  });

  const activePlaylist = $derived(
    playlists.find((p) => p.id === selectedId) ?? playlists[0] ?? null
  );

  async function handleCreate(e: Event) {
    e.preventDefault();
    if (!newTitle.trim()) return;
    await onCreatePlaylist(newTitle.trim());
    newTitle = '';
  }
</script>

<section class="playlists-view">
  <aside class="pl-sidebar">
    <form class="create-form" onsubmit={handleCreate}>
      <input type="text" bind:value={newTitle} placeholder="Новая подборка..." />
      <button type="submit" aria-label="Создать плейлист">
        <Plus size={15} />
      </button>
    </form>

    <div class="pl-list">
      {#each playlists as pl, pIdx (`${pl.id}-${pIdx}`)}
        <button
          class="pl-item"
          class:active={activePlaylist?.id === pl.id}
          onclick={() => (selectedId = pl.id)}
        >
          <ListMusic size={15} />
          <span class="pl-name">{pl.title}</span>
          <span class="pl-tag">{pl.isLocal ? 'Локальный' : pl.source}</span>
        </button>
      {/each}
    </div>
  </aside>

  <div class="pl-content">
    {#if activePlaylist}
      <div class="pl-head">
        <div>
          <h2>{activePlaylist.title}</h2>
          <p>
            {activePlaylist.isLocal
              ? 'Локальная мульти-сервисная подборка (Яндекс + Spotify + SoundCloud)'
              : 'Плейлист подключённого стримингового сервиса'}
          </p>
        </div>
        {#if activePlaylist.tracks.length > 0}
          <button
            class="play-all"
            onclick={() => onPlayTrack(activePlaylist.tracks[0], activePlaylist.tracks)}
          >
            <Play size={14} fill="currentColor" />
            <span>Слушать всё</span>
          </button>
        {/if}
      </div>

      {#if activePlaylist.tracks.length === 0}
        <div class="pl-empty">
          В этом плейлисте пока нет треков. Добавляйте треки из раздела Библиотеки с помощью кнопки «+».
        </div>
      {:else}
        <div class="pl-tracks">
          {#each activePlaylist.tracks as track, tIdx (`${track.id}-${tIdx}`)}
            <div class="pl-row">
              <button class="pl-play" onclick={() => onPlayTrack(track, activePlaylist.tracks)}>
                <img src={track.artwork} alt={track.title} />
              </button>
              <button type="button" class="pl-meta" onclick={() => onPlayTrack(track, activePlaylist.tracks)}>
                <div class="pl-t">{track.title}</div>
                <div class="pl-a">{track.artist}</div>
              </button>
              <SourceBadge source={track.source} />
              {#if activePlaylist.isLocal}
                <button
                  class="del-btn"
                  title="Удалить из подборки"
                  onclick={() => onRemoveTrack(activePlaylist.id, track.id)}
                >
                  <Trash2 size={14} />
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="pl-empty">Создайте первый локальный плейлист слева или подключите сервис в Settings.</div>
    {/if}
  </div>
</section>

<style>
  .playlists-view {
    height: 100%;
    display: grid;
    grid-template-columns: 260px 1fr;
  }

  .pl-sidebar {
    border-right: 1px solid var(--border-subtle);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: rgba(24, 25, 30, 0.5);
  }

  .create-form {
    display: flex;
    gap: 6px;
  }

  .create-form input {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-subtle);
    background: var(--bg-graphite);
    color: var(--text-primary);
    font-size: 12px;
  }

  .create-form button {
    width: 32px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #111215;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .pl-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .pl-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }

  .pl-item:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .pl-item.active {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .pl-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pl-tag {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .pl-content {
    padding: 20px 28px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .pl-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 14px;
  }

  .pl-head h2 {
    font-size: 20px;
    font-weight: 700;
  }

  .pl-head p {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 3px;
  }

  .play-all {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #111215;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .pl-tracks {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pl-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .pl-play {
    width: 38px;
    height: 38px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .pl-play img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .pl-meta {
    flex: 1;
    min-width: 0;
    cursor: pointer;
    border: none;
    background: transparent;
    text-align: left;
    color: inherit;
    padding: 0;
  }

  .pl-t {
    font-size: 13px;
    font-weight: 600;
  }

  .pl-a {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .del-btn {
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    padding: 6px;
  }

  .del-btn:hover {
    color: #fb7185;
  }

  .pl-empty {
    padding: 40px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }
</style>

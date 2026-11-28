<script lang="ts">
  import {
    Check,
    Copy,
    Crown,
    Globe,
    ListMusic,
    Lock,
    LogOut,
    Play,
    Pause,
    Radio,
    RefreshCw,
    SkipForward,
    Sparkles,
    Users,
    MessageCircle,
    Send,
    Wifi,
  } from 'lucide-svelte';
  import SourceBadge from './SourceBadge.svelte';
  import RadixSelect, { type SelectOption } from './ui/RadixSelect.svelte';
  import { pickDiverseSample, type JamRoomState, type PublicJamRoomInfo } from '../jamRoom';
  import { ROLE_BADGES } from '../supabase';
  import type { Playlist, Track } from '../types';
  import * as api from '../api';

  interface Props {
    room: JamRoomState;
    likedTracks: Track[];
    playlists: Playlist[];
    defaultName: string;
    onCreateRoom: (
      name: string,
      transport: 'mashiro' | 'lan',
      serverAddr: string,
      invite: string,
      lanPort: number,
      tracks: Track[]
    ) => void;
    onJoinRoom: (
      target: string,
      name: string,
      transport: 'mashiro' | 'lan',
      invite: string,
      lanPort: number,
      tracks: Track[]
    ) => void;
    onLeaveRoom: () => void;
    onUpdateTaste: (tasteLabel: string, tracks: Track[]) => void;
    onPlayTrack: (track: Track) => void;
    onTogglePlay: () => void;
    onSkip: () => void;
    onRebuildBlend: (freshTracks?: Track[]) => void;
    onSendChat?: (text: string) => void;
  }

  let {
    room,
    likedTracks,
    playlists,
    defaultName,
    onCreateRoom,
    onJoinRoom,
    onLeaveRoom,
    onUpdateTaste,
    onPlayTrack,
    onTogglePlay,
    onSkip,
    onRebuildBlend,
    onSendChat = () => {},
  }: Props = $props();

  let chatInput = $state('');

  let displayName = $state('');
  let joinCode = $state('');
  let roomTitle = $state('');
  let isPrivateRoom = $state(false);
  let transport = $state<'mashiro' | 'lan'>('mashiro');
  let mashiroAddr = $state('wss://your-relay.example/ws');
  let inviteCode = $state('');
  let lanPort = $state(7331);
  let lanHost = $state('192.168.0.100:7331');
  let tasteSource = $state<'liked' | 'recap' | string>('liked');
  let copied = $state(false);
  let loadingTaste = $state(false);

  const tasteOptions = $derived<SelectOption[]>([
    { value: 'liked', label: `Мне нравится (${likedTracks.length} тр.)` },
    { value: 'recap', label: 'Топ из Итогов месяца' },
    ...playlists.map((pl) => ({
      value: pl.id,
      label: `${pl.title} (${pl.tracks.length} тр.)`,
    })),
  ]);

  const currentTasteCount = $derived.by(() => {
    if (tasteSource === 'liked') return likedTracks.length;
    if (tasteSource === 'recap') return null;
    return playlists.find((p) => p.id === tasteSource)?.tracks.length ?? 0;
  });

  $effect(() => {
    if (defaultName) {
      displayName = defaultName;
    }
  });


  async function resolveTasteBundle(): Promise<{ label: string; tracks: Track[] }> {
    loadingTaste = true;
    try {
      if (tasteSource === 'liked') {
        return { label: 'Мне нравится', tracks: pickDiverseSample(likedTracks, 160) };
      }
      if (tasteSource === 'recap') {
        const recap = await api.getListeningRecap('month');
        const tracks = recap.topTracks.map((t) => t.track);
        return {
          label: 'Топ из Итогов',
          tracks: tracks.length > 0 ? tracks : pickDiverseSample(likedTracks, 100),
        };
      }
      const pl = playlists.find((p) => p.id === tasteSource);
      if (pl) {
        return { label: pl.title, tracks: pickDiverseSample(pl.tracks, 160) };
      }
      return { label: 'Мне нравится', tracks: pickDiverseSample(likedTracks, 160) };
    } finally {
      loadingTaste = false;
    }
  }

  async function handleCreate() {
    const cleanName = displayName.trim() || 'Хост';
    const taste = await resolveTasteBundle();
    onCreateRoom(cleanName, transport, mashiroAddr.trim(), inviteCode.trim(), lanPort, taste.tracks);
  }

  async function handleJoinByCode(rawCode: string) {
    const code = rawCode.trim();
    if (!code) return;
    const cleanName = displayName.trim() || 'Гость';
    const taste = await resolveTasteBundle();
    onJoinRoom(code, cleanName, transport, code, lanPort, taste.tracks);
  }

  async function handleJoin() {
    await handleJoinByCode(joinCode);
  }

  async function handleChangeTaste(nextSource: string) {
    tasteSource = nextSource;
    if (room.connected) {
      const taste = await resolveTasteBundle();
      onUpdateTaste(taste.label, taste.tracks);
    }
  }

  function copyCode() {
    if (!room.roomCode) return;
    navigator.clipboard?.writeText(room.roomCode).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const votesNeeded = $derived(Math.max(1, Math.ceil(room.members.length / 2)));
  const alreadyVoted = $derived(room.skipVotes.includes(room.myId));
</script>

<section class="room-view tab-enter">
  <div class="room-head">
    <div class="head-title">
      <Radio size={20} />
      <div>
        <h2>Общая комната (Jam)</h2>
        <p>Совмещайте плейлисты с друзьями и слушайте общий микс синхронно</p>
      </div>
    </div>
    {#if room.connected}
      <button type="button" class="leave-btn" onclick={onLeaveRoom}>
        <LogOut size={14} />
        <span>Выйти</span>
      </button>
    {/if}
  </div>

  {#if !room.connected}
    <div class="lobby-grid">
      <div class="lobby-card">
        <h3>Профиль и источник вкусов</h3>
        <label class="field">
          <span>Твоё имя в комнате</span>
          <input type="text" bind:value={displayName} placeholder="Например: Квики" />
        </label>

        <div class="field">
          <span>Что добавить в общий микс</span>
          <RadixSelect
            value={tasteSource}
            options={tasteOptions}
            onValueChange={(val) => handleChangeTaste(val)}
          />
        </div>

        <div class="field">
          <span>Транспорт комнаты</span>
          <div class="preset-row">
            <button
              type="button"
              class="preset-pill"
              class:active={transport === 'mashiro'}
              onclick={() => (transport = 'mashiro')}
            >
              Релей mashiro (интернет)
            </button>
            <button
              type="button"
              class="preset-pill"
              class:active={transport === 'lan'}
              onclick={() => (transport = 'lan')}
            >
              Локальная сеть / Radmin
            </button>
          </div>
        </div>
        {#if transport === 'mashiro'}
          <label class="field">
            <span>Адрес релея (wss://...)</span>
            <input
              type="text"
              bind:value={mashiroAddr}
              placeholder="wss://your-relay.example/ws"
            />
          </label>
          <label class="field">
            <span>Код-приглашение (пусто — создаст новая комната)</span>
            <input
              type="text"
              bind:value={inviteCode}
              placeholder="voyage sample crisp fever mimic rally"
            />
          </label>
        {:else}
          <label class="field">
            <span>Порт комнаты (по умолчанию 7331)</span>
            <input type="number" bind:value={lanPort} min={1} max={65535} />
          </label>
        {/if}
      </div>

      <div class="lobby-card actions-card">
        <h3>Создать или войти</h3>
        <p class="hint">
          Права в комнате: <b>управляет только хост</b> — он запускает, переключает и ставит треки на
          паузу, а остальные слушают синхронно.
        </p>

        <div class="field">
          <span>Доступ к комнате</span>
          <div class="preset-row">
            <button
              type="button"
              class="preset-pill"
              class:active={!isPrivateRoom}
              onclick={() => (isPrivateRoom = false)}
            >
              <Globe size={13} />
              <span>Общая</span>
            </button>
            <button
              type="button"
              class="preset-pill"
              class:active={isPrivateRoom}
              onclick={() => (isPrivateRoom = true)}
            >
              <Lock size={13} />
              <span>Приватная (по коду)</span>
            </button>
          </div>
        </div>

        {#if !isPrivateRoom}
          <label class="field">
            <span>Название комнаты (необязательно)</span>
            <input type="text" bind:value={roomTitle} placeholder="Например: Ночной чилл" />
          </label>
        {/if}

        <button type="button" class="btn-primary" disabled={loadingTaste} onclick={handleCreate}>
          <Sparkles size={15} />
          <span>{loadingTaste ? 'Подготовка...' : 'Создать комнату'}</span>
        </button>

        <div class="divider"><span>или войти к другу</span></div>

        <form
          class="join-row"
          onsubmit={(e) => {
            e.preventDefault();
            handleJoin();
          }}
        >
          <input
            type="text"
            bind:value={joinCode}
            placeholder={transport === 'mashiro' ? 'Код-приглашение' : 'Адрес хоста (192.168.x.x:7331)'}
          />
          <button type="submit" class="btn-secondary" disabled={loadingTaste || !joinCode.trim()}>
            Войти
          </button>
        </form>

        {#if room.statusText}
          <div class="status-msg">{room.statusText}</div>
        {/if}
      </div>
    </div>

  {:else}
    <!-- Активная комната -->
    <div class="active-top">
      <div class="code-box">
        {#if room.roomTitle}
          <span class="room-name-pill">{room.roomTitle}</span>
        {/if}
        <span class="privacy-badge" class:priv={room.isPrivate}>
          {#if room.isPrivate}
            <Lock size={11} /> Приватная
          {:else}
            <Globe size={11} /> Общая
          {/if}
        </span>
        <span class="code-lbl">Код</span>
        <div class="code-val-row">
          <strong>{room.roomCode}</strong>
          <button type="button" class="icon-btn" onclick={copyCode} title="Скопировать код">
            {#if copied}
              <Check size={14} />
            {:else}
              <Copy size={14} />
            {/if}
          </button>
        </div>
        <span class="role-badge" class:host={room.isHost}>
          {#if room.isHost}
            <Crown size={12} /> Хост (управление)
          {:else}
            <Users size={12} /> Гость (синхронно)
          {/if}
        </span>
        {#if !room.isHost}
          <span
            class="sync-badge"
            class:catching={room.syncStatus === 'catching-up'}
            class:away={room.syncStatus === 'host-away'}
          >
            <Wifi size={11} />
            <span>
              {room.syncStatus === 'catching-up'
                ? 'Догоняем...'
                : room.syncStatus === 'host-away'
                  ? 'Хост отошёл'
                  : 'Синхронизировано'}
            </span>
          </span>
        {/if}
      </div>

      <div class="contrib-bar">
        <div class="contrib-label">
          <ListMusic size={14} />
          <span>Мой вклад</span>
          {#if currentTasteCount !== null}
            <span class="contrib-count">{currentTasteCount} тр.</span>
          {/if}
        </div>
        <div class="contrib-select">
          <RadixSelect
            value={tasteSource}
            options={tasteOptions}
            onValueChange={(val) => handleChangeTaste(val)}
          />
        </div>
        {#if room.isHost}
          <button
            type="button"
            class="btn-sm"
            onclick={async () => {
              const taste = await resolveTasteBundle();
              onRebuildBlend(taste.tracks);
            }}
            title="Пересобрать микс из свежих треков источника"
          >
            <RefreshCw size={13} />
            <span>Пересобрать микс</span>
          </button>
        {/if}
      </div>
    </div>

    <!-- Участники -->
    <div class="members-strip">
      {#each room.members as m (m.id)}
        <div class="member-pill" class:me={m.id === room.myId}>
          {#if m.avatarUrl}
            <img class="avatar-img" src={m.avatarUrl} alt={m.name} />
          {:else}
            <span class="avatar">{m.name.slice(0, 1).toUpperCase()}</span>
          {/if}
          <div class="m-info">
            <div class="m-name">
              {m.name}{#if m.tag}<span class="m-tag">#{m.tag}</span>{/if}
              {#if m.role && ROLE_BADGES[m.role]}
                {@const rb = ROLE_BADGES[m.role]}
                {#if rb}
                  <span class="m-role" style={`color: ${rb.color}; background: ${rb.bg}`}>
                    {rb.label}
                  </span>
                {/if}
              {/if}
              {#if m.isHost}
                <Crown size={11} class="crown-ico" />
              {/if}
            </div>
            <div class="m-sub">{m.tasteLabel} · {m.trackCount} тр.</div>
          </div>
        </div>
      {/each}
    </div>

    <!-- Сейчас играет в комнате -->
    {#if room.currentTrack}
      {@const cur = room.currentTrack}
      <div class="now-sync-card">
        <img src={cur.artwork} alt={cur.title} />
        <div class="now-meta">
          <span class="now-tag">Сейчас играет в комнате</span>
          <div class="now-title">{cur.title}</div>
          <div class="now-artist">{cur.artist}</div>
        </div>
        <div class="now-actions">
          {#if room.isHost}
            <button type="button" class="btn-circle" onclick={onTogglePlay} aria-label="Пауза / Плей">
              {#if room.isPlaying}
                <Pause size={16} fill="currentColor" />
              {:else}
                <Play size={16} fill="currentColor" />
              {/if}
            </button>
            <button type="button" class="btn-skip" onclick={onSkip}>
              <SkipForward size={14} />
              <span>Скип</span>
            </button>
          {:else}
            <span class="host-only-pill">
              <Crown size={12} />
              <span>Управляет хост</span>
            </span>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Общая смешанная очередь -->
    <div class="queue-card">
      <div class="q-head">
        <span>Общий микс ({room.queue.length} треков)</span>
        {#if room.statusText}
          <span class="q-status">{room.statusText}</span>
        {/if}
      </div>

      {#if room.queue.length === 0}
        <div class="q-empty">
          Добавьте треки или дождитесь подключения друзей — микс соберётся автоматически.
        </div>
      {:else}
        <div class="q-list">
          {#each room.queue as item, idx (item.track.id + idx)}
            {@const active = room.currentTrack?.id === item.track.id}
            {#if room.isHost}
              <button
                type="button"
                class="q-row"
                class:active
                class:matched={item.matchCount >= 2}
                onclick={() => onPlayTrack(item.track)}
              >
                <span class="q-idx">{idx + 1}</span>
                <img src={item.track.artwork} alt={item.track.title} />
                <div class="q-meta">
                  <div class="q-title">{item.track.title}</div>
                  <div class="q-sub">{item.track.artist}</div>
                </div>
                <span class="q-reason" class:gold={item.matchCount >= 2}>{item.reason}</span>
                <SourceBadge source={item.track.source} showLabel={false} />
              </button>
            {:else}
              <div
                class="q-row readonly"
                class:active
                class:matched={item.matchCount >= 2}
              >
                <span class="q-idx">{idx + 1}</span>
                <img src={item.track.artwork} alt={item.track.title} />
                <div class="q-meta">
                  <div class="q-title">{item.track.title}</div>
                  <div class="q-sub">{item.track.artist}</div>
                </div>
                <span class="q-reason" class:gold={item.matchCount >= 2}>{item.reason}</span>
                <SourceBadge source={item.track.source} showLabel={false} />
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </div>

    <!-- Чат комнаты -->
    <div class="chat-card">
      <div class="q-head">
        <span style="display:inline-flex;align-items:center;gap:6px;"><MessageCircle size={14} /> Чат комнаты</span>
      </div>
      <div class="chat-list">
        {#if !room.chat || room.chat.length === 0}
          <div class="chat-empty">Напишите что-нибудь участникам комнаты...</div>
        {:else}
          {#each room.chat as msg (msg.id)}
            <div class="chat-msg" class:mine={msg.senderId === room.myId}>
              <span class="chat-author">{msg.senderName}:</span>
              <span class="chat-text">{msg.text}</span>
            </div>
          {/each}
        {/if}
      </div>
      <form
        class="chat-form"
        onsubmit={(e) => {
          e.preventDefault();
          if (!chatInput.trim()) return;
          onSendChat(chatInput);
          chatInput = '';
        }}
      >
        <input
          type="text"
          bind:value={chatInput}
          placeholder="Сообщение в комнату..."
          maxlength="240"
        />
        <button type="submit" class="btn-sm" aria-label="Отправить">
          <Send size={13} />
        </button>
      </form>
    </div>
  {/if}
</section>

<style>
  .room-view {
    padding: 20px 28px 34px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .room-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
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

  .leave-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 999px;
    border: 1px solid rgba(255, 77, 109, 0.35);
    background: rgba(255, 77, 109, 0.12);
    color: #ff6b86;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .lobby-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }

  .lobby-card {
    padding: 18px 20px;
    border-radius: 16px;
    background: rgba(19, 19, 24, 0.76);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .lobby-card h3 {
    font-size: 15px;
    font-weight: 700;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .field input,
  .join-row input {
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.7);
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
  }

  .preset-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .preset-pill {
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
  }

  .preset-pill.active {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .hint {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .btn-primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 11px 16px;
    border-radius: 12px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 13px;
    font-weight: 800;
    cursor: pointer;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .divider::before,
  .divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border-subtle);
  }

  .join-row {
    display: flex;
    gap: 8px;
  }

  .join-row input {
    flex: 1;
    text-transform: uppercase;
  }

  .btn-secondary,
  .btn-sm {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .status-msg {
    font-size: 12px;
    color: var(--accent);
  }

  .active-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
    padding: 14px 18px;
    border-radius: 16px;
    background: rgba(19, 19, 24, 0.76);
    border: 1px solid var(--border-subtle);
  }

  .code-box {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .code-lbl {
    font-size: 11px;
    color: var(--text-muted);
  }

  .code-val-row {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
  }

  .code-val-row strong {
    font-size: 15px;
    letter-spacing: 0.06em;
  }

  .icon-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    display: grid;
    place-items: center;
  }

  .role-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 700;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-secondary);
  }

  .role-badge.host {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
  }

  .members-strip {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .member-pill {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 12px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
  }

  .member-pill.me {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--accent);
    color: #0b0b0e;
    font-weight: 800;
    font-size: 13px;
    display: grid;
    place-items: center;
  }

  .avatar-img {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    object-fit: cover;
  }

  .m-tag {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .m-role {
    font-size: 9.5px;
    font-weight: 800;
    padding: 1px 6px;
    border-radius: 999px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .m-name {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    font-weight: 700;
  }

  :global(.crown-ico) {
    color: #fbbf24;
  }

  .m-sub {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .now-sync-card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: 16px;
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--accent) 18%, rgba(19, 19, 24, 0.88)),
      rgba(19, 19, 24, 0.86)
    );
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .now-sync-card img {
    width: 56px;
    height: 56px;
    border-radius: 10px;
    object-fit: cover;
  }

  .now-meta {
    flex: 1;
    min-width: 0;
  }

  .now-tag {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .now-title {
    font-size: 15px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .now-artist {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .now-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-circle {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .btn-skip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 13px;
    border-radius: 999px;
    border: 1px solid var(--border-subtle);
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .queue-card {
    padding: 16px 18px;
    border-radius: 16px;
    background: rgba(19, 19, 24, 0.72);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .q-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 14px;
    font-weight: 700;
  }

  .q-status {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .q-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .q-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
  }

  .q-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .q-row.active {
    background: var(--bg-elevated);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .q-idx {
    width: 22px;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
  }

  .q-row img {
    width: 36px;
    height: 36px;
    border-radius: 6px;
    object-fit: cover;
  }

  .q-meta {
    flex: 1;
    min-width: 0;
  }

  .q-title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .q-sub {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .q-reason {
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .q-reason.gold {
    background: rgba(251, 191, 36, 0.16);
    color: #fbbf24;
    font-weight: 600;
  }

  .q-empty {
    padding: 24px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .sync-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    background: rgba(16, 185, 129, 0.14);
    color: #34d399;
    border: 1px solid rgba(16, 185, 129, 0.35);
    width: fit-content;
  }

  .sync-badge.catching {
    background: rgba(99, 102, 241, 0.16);
    color: #a5b4fc;
    border-color: rgba(99, 102, 241, 0.35);
  }

  .sync-badge.away {
    background: rgba(245, 158, 11, 0.16);
    color: #fbbf24;
    border-color: rgba(245, 158, 11, 0.35);
  }

  .chat-card {
    padding: 14px 16px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .chat-list {
    max-height: 170px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .chat-empty {
    font-size: 12px;
    color: var(--text-muted);
    padding: 8px 0;
  }

  .chat-msg {
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--text-primary);
  }

  .chat-author {
    font-weight: 700;
    color: var(--accent);
    margin-right: 5px;
  }

  .chat-msg.mine .chat-author {
    color: #34d399;
  }

  .chat-form {
    display: flex;
    gap: 8px;
  }

  .chat-form input {
    flex: 1;
    min-width: 0;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.7);
    color: var(--text-primary);
    font-size: 12.5px;
    outline: none;
  }

  .room-name-pill {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .privacy-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 9px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    background: rgba(52, 211, 153, 0.12);
    color: #34d399;
    border: 1px solid rgba(52, 211, 153, 0.25);
  }

  .privacy-badge.priv {
    background: rgba(251, 191, 36, 0.12);
    color: #fbbf24;
    border-color: rgba(251, 191, 36, 0.25);
  }

  .contrib-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 8px 12px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid var(--border-subtle);
  }

  .contrib-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .contrib-count {
    padding: 2px 7px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 700;
  }

  .contrib-select {
    min-width: 190px;
  }

  .host-only-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    font-size: 11.5px;
    font-weight: 600;
  }

  .q-row.readonly {
    cursor: default;
  }

  .q-row.readonly:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .public-rooms-card {
    padding: 16px 18px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .pub-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .pub-title {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 13.5px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .pub-count {
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 700;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-secondary);
  }

  .pub-empty {
    font-size: 12.5px;
    color: var(--text-muted);
  }

  .pub-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .pub-room-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid var(--border-subtle);
  }

  .pub-art {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .pub-art-placeholder {
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-secondary);
  }

  .pub-meta {
    flex: 1;
    min-width: 0;
  }

  .pub-name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .pub-code {
    padding: 2px 6px;
    border-radius: 6px;
    font-size: 10.5px;
    font-weight: 600;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-secondary);
    letter-spacing: 0.04em;
  }

  .pub-sub {
    font-size: 11.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>

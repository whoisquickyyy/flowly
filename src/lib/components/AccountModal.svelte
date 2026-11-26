<script lang="ts">
  import { Crop, LogIn, LogOut, Save, Upload, User, UserPlus, X } from 'lucide-svelte';
  import * as sb from '../supabase';
  import type { FlowlyProfile, SupabaseSession } from '../supabase';

  interface Props {
    open: boolean;
    session: SupabaseSession | null;
    profile: FlowlyProfile | null;
    onClose: () => void;
    onAuthChange: (session: SupabaseSession | null, profile: FlowlyProfile | null) => void;
    onSyncCloudSettings?: () => Promise<void>;
  }

  let { open, session, profile, onClose, onAuthChange, onSyncCloudSettings }: Props = $props();

  let mode = $state<'login' | 'register'>('login');
  let email = $state('');
  let password = $state('');
  let username = $state('');
  let editName = $state('');
  let editAvatar = $state('');
  let busy = $state(false);
  let error = $state('');
  let info = $state('');

  // Кадрирование загруженной аватарки
  let fileInput: HTMLInputElement | undefined = $state();
  let cropSrc = $state<string | null>(null);
  let cropImg: HTMLImageElement | undefined = $state();
  let cropZoom = $state(1);
  let cropX = $state(0);
  let cropY = $state(0);
  let dragging = $state(false);
  let dragStart = { x: 0, y: 0, ox: 0, oy: 0 };

  $effect(() => {
    if (profile) {
      editName = profile.username;
      editAvatar = profile.avatar_url || '';
    }
  });

  function handleFileSelect(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      cropSrc = String(reader.result);
      cropZoom = 1.2;
      cropX = 0;
      cropY = 0;
    };
    reader.readAsDataURL(file);
    input.value = '';
  }

  function onCropPointerDown(e: PointerEvent) {
    dragging = true;
    dragStart = { x: e.clientX, y: e.clientY, ox: cropX, oy: cropY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onCropPointerMove(e: PointerEvent) {
    if (!dragging) return;
    cropX = dragStart.ox + (e.clientX - dragStart.x);
    cropY = dragStart.oy + (e.clientY - dragStart.y);
  }

  function onCropPointerUp() {
    dragging = false;
  }

  function applyCrop() {
    if (!cropImg) return;
    const outSize = 160;
    const viewSize = 200;
    const canvas = document.createElement('canvas');
    canvas.width = outSize;
    canvas.height = outSize;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const nw = cropImg.naturalWidth;
    const nh = cropImg.naturalHeight;
    const baseScale = Math.max(viewSize / nw, viewSize / nh);
    const totalScale = baseScale * cropZoom * (outSize / viewSize);
    const drawW = nw * totalScale;
    const drawH = nh * totalScale;
    const dx = outSize / 2 - drawW / 2 + cropX * (outSize / viewSize);
    const dy = outSize / 2 - drawH / 2 + cropY * (outSize / viewSize);

    ctx.clearRect(0, 0, outSize, outSize);
    ctx.drawImage(cropImg, dx, dy, drawW, drawH);
    editAvatar = canvas.toDataURL('image/webp', 0.85);
    cropSrc = null;
  }

  async function handleLogin() {
    error = '';
    info = '';
    busy = true;
    try {
      const res = await sb.signIn(email, password);
      onAuthChange(res.session, res.profile);
      onClose();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function handleRegister() {
    error = '';
    info = '';
    busy = true;
    try {
      const res = await sb.signUp(email, password, username);
      if (res.needsEmailConfirm) {
        info = 'Проверьте почту для подтверждения регистрации, затем войдите.';
        mode = 'login';
      } else {
        onAuthChange(res.session, res.profile);
        onClose();
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function handleSaveProfile() {
    if (!session) return;
    error = '';
    info = '';
    busy = true;
    try {
      const updated = await sb.updateProfile(session, {
        username: editName.trim() || 'Слушатель',
        avatar_url: editAvatar.trim(),
      });
      onAuthChange(session, updated);
      info = 'Профиль обновлён';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function handleGuestLogin() {
    const prof = sb.signInAsGuest(username || 'Слушатель');
    onAuthChange(null, prof);
    onClose();
  }

  function handleLogout() {
    sb.signOut();
    onAuthChange(null, null);
    onClose();
  }

  const roleBadge = $derived(sb.ROLE_BADGES[profile?.role ?? 'listener'] ?? null);
</script>

{#if open}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
  >
    <div class="modal-card">
      <div class="modal-head">
        <div class="m-title">
          <User size={17} />
          <span>{profile ? 'Профиль Flowly' : 'Аккаунт Flowly'}</span>
        </div>
        <button type="button" class="close-btn" onclick={onClose} aria-label="Закрыть">
          <X size={16} />
        </button>
      </div>

      {#if !sb.isSupabaseConfigured()}
        <div class="warn-box">
          Supabase ещё не подключён. Вставьте <code>SUPABASE_URL</code> и{' '}
          <code>SUPABASE_ANON_KEY</code> в <code>src/lib/supabase.ts</code>.
        </div>
      {:else if cropSrc}
        <div class="crop-box">
          <div class="crop-hint">
            <Crop size={14} />
            <span>Перетащите фото и настройте масштаб</span>
          </div>
          <div
            class="crop-viewport"
            role="presentation"
            onpointerdown={onCropPointerDown}
            onpointermove={onCropPointerMove}
            onpointerup={onCropPointerUp}
          >
            <img
              bind:this={cropImg}
              src={cropSrc}
              alt="Кадрирование"
              draggable="false"
              style={`transform: translate(${cropX}px, ${cropY}px) scale(${cropZoom})`}
            />
          </div>
          <label class="zoom-row">
            <span>Масштаб</span>
            <input type="range" min="1" max="3.5" step="0.05" bind:value={cropZoom} />
          </label>
          <div class="modal-actions">
            <button type="button" class="btn-primary" onclick={applyCrop}>Применить кадр</button>
            <button type="button" class="btn-danger" onclick={() => (cropSrc = null)}>Отмена</button>
          </div>
        </div>
      {:else if session && profile}
        <div class="profile-preview">
          {#if editAvatar}
            <img class="p-avatar" src={editAvatar} alt={editName} />
          {:else}
            <div class="p-avatar placeholder">{(editName || 'F').slice(0, 1).toUpperCase()}</div>
          {/if}
          <div class="p-meta">
            <div class="p-name-row">
              <div class="p-name">{profile.username}<span>#{profile.tag}</span></div>
              {#if roleBadge}
                <span
                  class="role-tag"
                  style={`color: ${roleBadge.color}; background: ${roleBadge.bg}`}
                >
                  {roleBadge.label}
                </span>
              {/if}
            </div>
            <div class="p-email">{session.user.email}</div>
          </div>
        </div>

        <input
          type="file"
          accept="image/*"
          bind:this={fileInput}
          onchange={handleFileSelect}
          hidden
        />

        <label class="field">
          <span>Никнейм (виден в комнатах)</span>
          <input type="text" bind:value={editName} placeholder="Ваш никнейм" />
        </label>

        <div class="field">
          <span>Аватарка</span>
          <div class="avatar-row">
            <button type="button" class="btn-upload" onclick={() => fileInput?.click()}>
              <Upload size={14} />
              <span>Загрузить с ПК</span>
            </button>
            <input type="url" bind:value={editAvatar} placeholder="или ссылка https://..." />
          </div>
        </div>

        {#if error}<div class="msg err">{error}</div>{/if}
        {#if info}<div class="msg ok">{info}</div>{/if}

        <div class="modal-actions">
          <button type="button" class="btn-primary" disabled={busy} onclick={handleSaveProfile}>
            <Save size={14} />
            <span>{busy ? 'Сохранение...' : 'Сохранить'}</span>
          </button>
          {#if onSyncCloudSettings}
            <button
              type="button"
              class="btn-sm"
              disabled={busy}
              onclick={async () => {
                busy = true;
                error = '';
                info = '';
                try {
                  await onSyncCloudSettings();
                  info = 'Настройки сохранены в облако аккаунта';
                } catch (e) {
                  error = e instanceof Error ? e.message : String(e);
                } finally {
                  busy = false;
                }
              }}
            >
              <span>Сохранить настройки в облако</span>
            </button>
          {/if}
          <button type="button" class="btn-danger" onclick={handleLogout}>
            <LogOut size={14} />
            <span>Выйти</span>
          </button>
        </div>
      {:else}
        <div class="tabs">
          <button type="button" class:active={mode === 'login'} onclick={() => (mode = 'login')}>
            <LogIn size={13} /> Вход
          </button>
          <button type="button" class:active={mode === 'register'} onclick={() => (mode = 'register')}>
            <UserPlus size={13} /> Регистрация
          </button>
        </div>

        {#if mode === 'register'}
          <label class="field">
            <span>Никнейм</span>
            <input type="text" bind:value={username} placeholder="Как тебя будут видеть друзья" />
          </label>
        {/if}

        <label class="field">
          <span>Email</span>
          <input type="email" bind:value={email} placeholder="you@example.com" />
        </label>

        <label class="field">
          <span>Пароль</span>
          <input type="password" bind:value={password} placeholder="Минимум 6 символов" />
        </label>

        {#if error}<div class="msg err">{error}</div>{/if}
        {#if info}<div class="msg ok">{info}</div>{/if}

        <button
          type="button"
          class="btn-primary full"
          disabled={busy}
          onclick={mode === 'login' ? handleLogin : handleRegister}
        >
          <span>{busy ? 'Подождите...' : mode === 'login' ? 'Войти' : 'Создать аккаунт'}</span>
        </button>

        <button
          type="button"
          class="btn-sm full"
          disabled={busy}
          onclick={handleGuestLogin}
          style="justify-content: center; margin-top: 2px;"
        >
          <span>Быстрый вход без email (для Jam-комнат)</span>
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(5, 5, 9, 0.72);
    backdrop-filter: blur(10px);
    display: grid;
    place-items: center;
    padding: 20px;
  }

  .modal-card {
    width: 100%;
    max-width: 420px;
    padding: 20px;
    border-radius: 18px;
    background: rgba(18, 18, 24, 0.96);
    border: 1px solid var(--border-subtle);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.65);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .m-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .close-btn {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    padding: 4px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
  }

  .tabs button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px;
    border-radius: 9px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .tabs button.active {
    background: var(--accent);
    color: #0b0b0e;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .field input {
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: rgba(10, 10, 14, 0.75);
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
  }

  .avatar-row {
    display: flex;
    gap: 8px;
  }

  .avatar-row input {
    flex: 1;
    min-width: 0;
  }

  .btn-upload {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }

  .crop-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .crop-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .crop-viewport {
    width: 200px;
    height: 200px;
    border-radius: 50%;
    overflow: hidden;
    border: 2px solid var(--accent);
    background: #0b0b0e;
    display: grid;
    place-items: center;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }

  .crop-viewport:active {
    cursor: grabbing;
  }

  .crop-viewport img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }

  .zoom-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .zoom-row input {
    flex: 1;
    accent-color: var(--accent);
  }

  .profile-preview {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
  }

  .p-avatar {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
  }

  .p-avatar.placeholder {
    display: grid;
    place-items: center;
    background: var(--accent);
    color: #0b0b0e;
    font-weight: 800;
    font-size: 17px;
  }

  .p-meta {
    min-width: 0;
  }

  .p-name-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .p-name {
    font-size: 15px;
    font-weight: 700;
  }

  .p-name span {
    color: var(--accent);
    font-size: 12px;
    margin-left: 2px;
  }

  .role-tag {
    font-size: 10px;
    font-weight: 800;
    padding: 2px 7px;
    border-radius: 999px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .p-email {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .modal-actions {
    width: 100%;
    display: flex;
    gap: 8px;
  }

  .btn-primary {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 14px;
    border-radius: 10px;
    border: none;
    background: var(--accent);
    color: #0b0b0e;
    font-size: 13px;
    font-weight: 800;
    cursor: pointer;
  }

  .btn-primary.full {
    width: 100%;
  }

  .btn-danger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 14px;
    border-radius: 10px;
    border: 1px solid rgba(255, 77, 109, 0.35);
    background: rgba(255, 77, 109, 0.12);
    color: #ff6b86;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .msg {
    font-size: 12px;
    padding: 8px 10px;
    border-radius: 8px;
  }

  .msg.err {
    background: rgba(255, 77, 109, 0.14);
    color: #ff859b;
  }

  .msg.ok {
    background: rgba(16, 185, 129, 0.14);
    color: #34d399;
  }

  .warn-box {
    font-size: 12.5px;
    line-height: 1.5;
    padding: 12px;
    border-radius: 10px;
    background: rgba(245, 158, 11, 0.12);
    color: #fbbf24;
  }
</style>

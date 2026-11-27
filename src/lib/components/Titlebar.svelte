<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { Minus, Square, X, Disc3 } from 'lucide-svelte';
  import { isTauri } from '../api';

  function minimize() {
    if (isTauri()) getCurrentWindow().minimize();
  }
  function toggleMax() {
    if (isTauri()) getCurrentWindow().toggleMaximize();
  }
  function close() {
    if (isTauri()) getCurrentWindow().close();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <div class="logo-ring">
      <Disc3 size={16} />
    </div>
    <span class="brand-name">Aura</span>
    <span class="brand-badge">RU Split-Proxy</span>
  </div>

  <div class="win-controls">
    <button class="win-btn" onclick={minimize} aria-label="Свернуть">
      <Minus size={14} />
    </button>
    <button class="win-btn" onclick={toggleMax} aria-label="Развернуть">
      <Square size={12} />
    </button>
    <button class="win-btn close" onclick={close} aria-label="Закрыть">
      <X size={14} />
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px 0 16px;
    border-bottom: 1px solid var(--border-subtle);
    background: rgba(12, 12, 18, 0.92);
    position: relative;
    z-index: 30;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .logo-ring {
    width: 24px;
    height: 24px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #8b5cf6, #ec4899);
    color: white;
    box-shadow: 0 0 14px rgba(139, 92, 246, 0.45);
  }

  .brand-name {
    font-weight: 700;
    font-size: 13px;
    letter-spacing: 0.04em;
  }

  .brand-badge {
    font-size: 10px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    border: 1px solid var(--border-subtle);
  }

  .win-controls {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .win-btn {
    width: 30px;
    height: 26px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .win-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .win-btn.close:hover {
    background: #e11d48;
    color: white;
  }
</style>

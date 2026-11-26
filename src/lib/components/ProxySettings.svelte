<script lang="ts">
  import { ShieldCheck, Wifi, WifiOff, RefreshCw, Save } from 'lucide-svelte';
  import type { ProxyConfig, ProxyMode, SourceKind, SourceStatus } from '../types';

  interface Props {
    sources: SourceStatus[];
    onSaveProxy: (source: SourceKind, config: ProxyConfig) => Promise<void>;
    onTestConnection: (source: SourceKind) => Promise<void>;
  }

  let { sources, onSaveProxy, onTestConnection }: Props = $props();

  let testing = $state<Record<string, boolean>>({});
  let saving = $state<Record<string, boolean>>({});
  let drafts = $state<Record<string, ProxyConfig>>({});

  $effect(() => {
    for (const src of sources) {
      if (!drafts[src.source]) {
        drafts[src.source] = { ...src.proxy };
      }
    }
  });

  function setMode(source: SourceKind, mode: ProxyMode) {
    const current = drafts[source] ?? { mode: 'direct', host: '127.0.0.1', port: 1080 };
    drafts[source] = {
      ...current,
      mode,
      host: mode !== 'direct' && !current.host ? '127.0.0.1' : current.host,
      port: mode !== 'direct' && !current.port ? 1080 : current.port,
    };
  }

  async function handleSave(source: SourceKind) {
    const cfg = drafts[source];
    if (!cfg) return;
    saving[source] = true;
    try {
      await onSaveProxy(source, cfg);
    } finally {
      saving[source] = false;
    }
  }

  async function handleTest(source: SourceKind) {
    testing[source] = true;
    try {
      await onTestConnection(source);
    } finally {
      testing[source] = false;
    }
  }

  function sourceAccent(source: SourceKind) {
    if (source === 'yandex') return '#f59e0b';
    if (source === 'spotify') return '#10b981';
    return '#f97316';
  }
</script>

<section class="proxy-page">
  <div class="header-box">
    <div class="icon-wrap">
      <ShieldCheck size={22} />
    </div>
    <div>
      <h2>Split-Tunneling и обход ограничений</h2>
      <p>
        Настрой маршрутизацию отдельно для каждого сервиса: Яндекс Музыка работает напрямую без VPN,
        а Spotify и SoundCloud можно пустить через локальный SOCKS5/HTTP прокси (Xray, NekoBox, Amnezia).
      </p>
    </div>
  </div>

  <div class="cards-grid">
    {#each sources as src (src.source)}
      {@const draft = drafts[src.source] ?? src.proxy}
      <div class="card" style="--card-accent: {sourceAccent(src.source)}">
        <div class="card-top">
          <div class="source-title">
            <span class="dot"></span>
            <h3>{src.name}</h3>
          </div>

          <div class="status-pill" class:ok={src.is_reachable === true} class:err={src.is_reachable === false}>
            {#if src.is_reachable === true}
              <Wifi size={13} />
              <span>Доступен · {src.latency_ms} мс</span>
            {:else if src.is_reachable === false}
              <WifiOff size={13} />
              <span>Недоступен</span>
            {:else}
              <span>Не проверялся</span>
            {/if}
          </div>
        </div>

        <div class="mode-switch">
          <button
            class="mode-btn"
            class:active={draft.mode === 'direct'}
            onclick={() => setMode(src.source, 'direct')}
          >
            Напрямую (Direct)
          </button>
          <button
            class="mode-btn"
            class:active={draft.mode === 'socks5'}
            onclick={() => setMode(src.source, 'socks5')}
          >
            SOCKS5
          </button>
          <button
            class="mode-btn"
            class:active={draft.mode === 'http'}
            onclick={() => setMode(src.source, 'http')}
          >
            HTTP
          </button>
        </div>

        {#if draft.mode !== 'direct'}
          <div class="fields">
            <label class="field">
              <span>Хост</span>
              <input type="text" bind:value={draft.host} placeholder="127.0.0.1" />
            </label>
            <label class="field port">
              <span>Порт</span>
              <input type="number" bind:value={draft.port} min="1" max="65535" />
            </label>
          </div>
        {:else}
          <p class="direct-hint">
            Запросы идут напрямую в обход системного прокси — оптимально для российских сервисов.
          </p>
        {/if}

        {#if src.last_error}
          <div class="error-box">{src.last_error}</div>
        {/if}

        <div class="card-actions">
          <button
            class="btn secondary"
            disabled={testing[src.source]}
            onclick={() => handleTest(src.source)}
          >
            <RefreshCw size={14} class={testing[src.source] ? 'spin' : ''} />
            <span>{testing[src.source] ? 'Проверка...' : 'Проверить'}</span>
          </button>

          <button
            class="btn primary"
            disabled={saving[src.source]}
            onclick={() => handleSave(src.source)}
          >
            <Save size={14} />
            <span>{saving[src.source] ? 'Сохранение...' : 'Применить'}</span>
          </button>
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  .proxy-page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .header-box {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 18px 20px;
    border-radius: 16px;
    background: rgba(139, 92, 246, 0.08);
    border: 1px solid rgba(139, 92, 246, 0.25);
  }

  .icon-wrap {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: rgba(139, 92, 246, 0.2);
    color: #c4b5fd;
    flex-shrink: 0;
  }

  .header-box h2 {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 4px;
  }

  .header-box p {
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 16px;
  }

  .card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 16px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    backdrop-filter: blur(12px);
  }

  .card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .source-title {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--card-accent);
    box-shadow: 0 0 10px var(--card-accent);
  }

  .source-title h3 {
    font-size: 15px;
    font-weight: 600;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    padding: 4px 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
  }

  .status-pill.ok {
    background: rgba(16, 185, 129, 0.14);
    color: #34d399;
  }

  .status-pill.err {
    background: rgba(244, 63, 94, 0.14);
    color: #fb7185;
  }

  .mode-switch {
    display: grid;
    grid-template-columns: 1.4fr 1fr 1fr;
    gap: 4px;
    padding: 4px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid var(--border-subtle);
  }

  .mode-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    padding: 7px 6px;
    border-radius: 7px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .mode-btn.active {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .fields {
    display: flex;
    gap: 10px;
  }

  .field {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field.port {
    flex: 0 0 92px;
  }

  .field span {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .field input {
    width: 100%;
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px solid var(--border-subtle);
    background: rgba(0, 0, 0, 0.35);
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
  }

  .field input:focus {
    border-color: var(--card-accent);
  }

  .direct-hint {
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-muted);
    padding: 6px 0;
  }

  .error-box {
    font-size: 11px;
    color: #fda4af;
    background: rgba(244, 63, 94, 0.1);
    border: 1px solid rgba(244, 63, 94, 0.25);
    padding: 8px 10px;
    border-radius: 8px;
    word-break: break-word;
  }

  .card-actions {
    margin-top: auto;
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 9px;
    font-size: 12px;
    font-weight: 600;
    border: none;
    cursor: pointer;
    transition: transform 0.12s ease, opacity 0.15s ease, background 0.15s ease;
  }

  .btn:disabled {
    opacity: 0.55;
    cursor: wait;
  }

  .btn.secondary {
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-primary);
  }

  .btn.secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.13);
  }

  .btn.primary {
    background: var(--card-accent);
    color: #0b0b0f;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>

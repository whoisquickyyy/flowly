<script lang="ts">
  import { CheckCircle2, ExternalLink, Sparkles, X } from 'lucide-svelte';
  import RadixDialog from './ui/RadixDialog.svelte';
  import FlowlyLogo from './FlowlyLogo.svelte';
  import * as api from '../api';

  interface Props {
    open: boolean;
    version: string;
    notes?: string;
    date?: string | null;
    onClose: () => void;
  }

  let { open, version, notes = '', date = null, onClose }: Props = $props();

  const DEFAULT_NOTES = [
    'Обновлённый раздел настроек: отдельные вкладки «Оформление», «Звук», «Профиль» и «О программе»',
    'Визуальные мини-превью для всех тем оформления и быстрый выбор динамической темы',
    'Автоматическая проверка обновлений при каждом запуске с просмотром списка изменений',
    'Улучшена стабильность воспроизведения и исправлены ошибки интерфейса',
  ];

  const parsedLines = $derived.by(() => {
    const raw = notes.trim();
    if (!raw) return DEFAULT_NOTES;
    const lines = raw
      .split(/\r?\n/)
      .map((l) => l.trim().replace(/^[-*•]\s+/, ''))
      .filter((l) => l.length > 0 && !l.startsWith('#'));
    return lines.length > 0 ? lines : DEFAULT_NOTES;
  });

  const formattedDate = $derived.by(() => {
    if (!date) return '';
    const d = new Date(date);
    if (Number.isNaN(d.getTime())) return '';
    return d.toLocaleDateString('ru-RU', {
      day: 'numeric',
      month: 'long',
      year: 'numeric',
    });
  });
</script>

<RadixDialog {open} onOpenChange={(v) => { if (!v) onClose(); }} maxWidth="540px">
  <div class="wn-top">
    <span class="wn-pill">
      <Sparkles size={12} />
      <span>Обновлено до v{version}</span>
    </span>
    <button type="button" class="wn-close" onclick={onClose}>
      <span>Закрыть</span>
      <X size={14} />
    </button>
  </div>

  <div class="wn-stage">
    <div class="wn-hero">
      <div class="wn-logo-wrap">
        <FlowlyLogo size={36} />
      </div>
      <div class="wn-hero-text">
        <h2>Что нового в Flowly v{version}</h2>
        <p>
          {#if formattedDate}
            Релиз от {formattedDate} · Спасибо, что остаётесь с нами!
          {:else}
            Приложение успешно обновлено · Спасибо, что остаётесь с нами!
          {/if}
        </p>
      </div>
    </div>
  </div>

  <div class="wn-list-box">
    <div class="wn-list-title">Список изменений</div>
    <div class="wn-list">
      {#each parsedLines as item}
        <div class="wn-item">
          <CheckCircle2 size={15} class="wn-item-icon" />
          <span>{item}</span>
        </div>
      {/each}
    </div>
  </div>

  <div class="wn-footer">
    <button
      type="button"
      class="wn-btn ghost"
      onclick={() => api.openExternalUrl('https://t.me/whoisquicky')}
    >
      <span>Telegram-канал</span>
      <ExternalLink size={13} />
    </button>
    <button type="button" class="wn-btn primary" onclick={onClose}>
      <span>Погнали слушать</span>
    </button>
  </div>
</RadixDialog>

<style>
  .wn-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .wn-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }

  .wn-close {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
  }

  .wn-close:hover {
    color: var(--text-primary);
  }

  .wn-stage {
    padding: 16px;
    border-radius: 14px;
    background:
      radial-gradient(circle at 85% 20%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 55%),
      linear-gradient(135deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.015));
    border: 1px solid var(--border-subtle);
  }

  .wn-hero {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .wn-logo-wrap {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    flex-shrink: 0;
  }

  .wn-hero-text h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 800;
    color: var(--text-primary);
  }

  .wn-hero-text p {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .wn-list-box {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-subtle);
    max-height: 240px;
    overflow-y: auto;
  }

  .wn-list-title {
    font-size: 11.5px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .wn-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .wn-item {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--text-secondary);
  }

  .wn-item :global(.wn-item-icon) {
    color: var(--accent);
    margin-top: 2px;
    flex-shrink: 0;
  }

  .wn-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 2px;
  }

  .wn-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 9px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .wn-btn.ghost {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--border-subtle);
    color: var(--text-secondary);
  }

  .wn-btn.ghost:hover {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text-primary);
  }

  .wn-btn.primary {
    background: var(--accent);
    color: #fff;
  }

  .wn-btn.primary:hover {
    filter: brightness(1.08);
  }
</style>

<script lang="ts">
  import type { SourceKind } from '../types';

  interface Props {
    source: SourceKind;
    showLabel?: boolean;
  }

  let { source, showLabel = true }: Props = $props();

  const meta = {
    yandex: { label: 'Yandex Music', color: '#f59e0b' },
    spotify: { label: 'Spotify', color: '#10b981' },
    soundcloud: { label: 'SoundCloud', color: '#f97316' },
    local: { label: 'Локально', color: '#a855f7' },
  } as const;
</script>

<span class="source-badge" style="--src-color: {meta[source].color}">
  {#if source === 'yandex'}
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="10" fill="var(--src-color)" />
      <path d="M13.6 6H11.8C9.6 6 8.3 7.3 8.3 9.2C8.3 10.7 9.1 11.7 10.5 12.6L8 18H10.1L12.7 12.2L11.7 11.6C10.7 11 10.2 10.2 10.2 9.1C10.2 8.1 10.9 7.5 12 7.5H12.4V18H14.2V6H13.6Z" fill="#111215" />
    </svg>
  {:else if source === 'spotify'}
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="10" fill="var(--src-color)" />
      <path d="M7.5 9.8C10.8 8.8 14.5 9.1 17.2 10.6M8.2 12.7C10.9 11.9 13.8 12.1 16 13.4M9 15.4C11.1 14.8 13.3 15 15 16" stroke="#111215" stroke-width="1.8" stroke-linecap="round" />
    </svg>
  {:else if source === 'local'}
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="2" y="2" width="20" height="20" rx="6" fill="var(--src-color)" />
      <path d="M7 9.5A1.5 1.5 0 0 1 8.5 8H11L12.5 9.5H15.5A1.5 1.5 0 0 1 17 11V15.5A1.5 1.5 0 0 1 15.5 17H8.5A1.5 1.5 0 0 1 7 15.5V9.5Z" fill="#111215" />
    </svg>
  {:else}
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="2" y="2" width="20" height="20" rx="6" fill="var(--src-color)" />
      <path d="M6 15V12M8.5 15V10.5M11 15V9M13.5 15V8.5C15.8 8.5 17.5 10 17.5 12.2C17.5 13.8 16.3 15 14.6 15H13.5Z" stroke="#111215" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  {/if}
  {#if showLabel}
    <span>{meta[source].label}</span>
  {/if}
</span>

<style>
  .source-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }
</style>

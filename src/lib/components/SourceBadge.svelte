<script lang="ts">
  import type { SourceKind } from '../types';

  interface Props {
    source: SourceKind;
    showLabel?: boolean;
  }

  let { source, showLabel = true }: Props = $props();

  const meta = {
    yandex: { label: 'Яндекс Музыка', color: '#fcca00' },
    soundcloud: { label: 'SoundCloud', color: '#f97316' },
    local: { label: 'Локально', color: '#a855f7' },
  } as const;
</script>

<span class="source-badge" style="--src-color: {meta[source].color}">
  {#if source === 'yandex'}
    <svg width="14" height="14" viewBox="0 0 165 165" fill="none" aria-hidden="true">
      <path d="M33.9834 16.8654C46.1223 7.87059 60.4694 2.43314 75.4224 1.08203V25.8256C65.7566 27.0787 56.5254 30.7794 48.6291 36.6305C38.7418 43.9569 31.4707 54.2669 27.8885 66.0399C24.3062 77.8129 24.6024 90.4254 28.7333 102.017C32.8642 113.609 40.6111 123.567 50.8315 130.421C61.0518 137.275 73.2043 140.663 85.4967 140.086C97.789 139.508 109.57 134.995 119.103 127.213C128.635 119.43 135.414 108.79 138.44 96.8622C140.551 88.542 140.757 79.9024 139.104 71.5733L159.882 54.8691L159.868 54.7344C165.477 70.1632 166.334 86.9485 162.285 102.911C157.962 119.951 148.278 135.151 134.66 146.269C121.042 157.386 104.212 163.833 86.6513 164.658C69.0909 165.483 51.7301 160.643 37.1296 150.851C22.5292 141.06 11.4621 126.835 5.56083 110.275C-0.340466 93.7153 -0.763563 75.6974 4.35394 58.8789C9.47145 42.0603 19.8586 27.3317 33.9834 16.8654Z" fill="#FCCA00" />
      <path d="M144.215 28.4141L144.302 28.6321L130.417 50.6853C125.367 43.2026 118.601 36.9743 110.682 32.5658V82.7519C110.682 98.1496 98.1999 110.632 82.8023 110.632C67.4046 110.632 54.9224 98.1496 54.9224 82.7519C54.9224 67.3543 67.4046 54.872 82.8023 54.872C88.5729 54.872 93.9341 56.6253 98.3822 59.6281V2.23047C116.483 5.71201 132.466 15.1453 144.215 28.4141Z" fill="#FC3F1D" />
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

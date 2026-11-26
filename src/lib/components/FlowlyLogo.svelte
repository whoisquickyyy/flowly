<script lang="ts">
  interface Props {
    size?: number;
  }

  let { size = 20 }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  class="flowly-logo"
  aria-hidden="true"
>
  <rect
    x="1.5"
    y="1.5"
    width="21"
    height="21"
    rx="6.5"
    fill="color-mix(in srgb, var(--accent) 18%, #16161c)"
    stroke="color-mix(in srgb, var(--accent) 55%, transparent)"
    stroke-width="1.25"
  />
  <path
    d="M6.5 14.5C8.2 9.5 10.2 9.5 12 12C13.8 14.5 15.8 14.5 17.5 9.5"
    stroke="var(--accent)"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <circle cx="17.5" cy="9.5" r="1.4" fill="var(--text-primary)" />
</svg>

<style>
  .flowly-logo {
    flex-shrink: 0;
    display: block;
    filter: drop-shadow(0 0 8px color-mix(in srgb, var(--accent) 30%, transparent));
  }
</style>

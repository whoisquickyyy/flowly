<script lang="ts">
  import Icon, { addCollection } from '@iconify/svelte';
  import solar from '@iconify-json/solar/icons.json';

  addCollection(solar as Parameters<typeof addCollection>[0]);

  interface Props {
    name: string;
    size?: number;
    class?: string;
  }

  let { name, size = 16, class: className = '' }: Props = $props();
</script>

<Icon icon="solar:{name}" width={size} height={size} class={className} />

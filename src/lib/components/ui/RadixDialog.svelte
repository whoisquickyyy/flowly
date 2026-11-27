<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    maxWidth?: string;
    children: Snippet;
  }

  let { open, onOpenChange, maxWidth = '460px', children }: Props = $props();
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Portal>
    <Dialog.Overlay class="rx-dialog-overlay" />
    <Dialog.Content class="rx-dialog-content" style={`--rx-dialog-max-w: ${maxWidth}`}>
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

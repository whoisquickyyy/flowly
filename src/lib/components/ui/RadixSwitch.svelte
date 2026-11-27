<script lang="ts">
  import { Switch } from 'bits-ui';

  interface Props {
    checked: boolean;
    disabled?: boolean;
    ariaLabel?: string;
    onCheckedChange: (checked: boolean) => void;
  }

  let { checked, disabled = false, ariaLabel = 'Переключатель', onCheckedChange }: Props = $props();
</script>

<Switch.Root
  {checked}
  {disabled}
  aria-label={ariaLabel}
  onCheckedChange={onCheckedChange}
  class="rx-switch-root"
>
  <Switch.Thumb class="rx-switch-thumb" />
</Switch.Root>

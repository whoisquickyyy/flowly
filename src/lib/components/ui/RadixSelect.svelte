<script lang="ts">
  import { Select } from 'bits-ui';
  import { Check, ChevronDown } from 'lucide-svelte';

  export interface SelectOption {
    value: string;
    label: string;
  }

  interface Props {
    value: string;
    options: SelectOption[];
    placeholder?: string;
    disabled?: boolean;
    onValueChange: (val: string) => void;
  }

  let {
    value,
    options,
    placeholder = 'Выберите...',
    disabled = false,
    onValueChange,
  }: Props = $props();

  const selectedLabel = $derived(
    options.find((o) => o.value === value)?.label ?? placeholder,
  );
</script>

<Select.Root
  type="single"
  {value}
  items={options}
  {disabled}
  onValueChange={(v) => {
    if (v !== undefined) onValueChange(v);
  }}
>
  <Select.Trigger class="rx-select-trigger">
    <span class="rx-select-val">{selectedLabel}</span>
    <ChevronDown size={14} class="rx-select-chev" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content class="rx-select-content" sideOffset={6}>
      <Select.Viewport class="rx-select-viewport">
        {#each options as opt (opt.value)}
          <Select.Item value={opt.value} label={opt.label} class="rx-select-item">
            {#snippet children({ selected })}
              <span>{opt.label}</span>
              {#if selected}
                <Check size={13} />
              {/if}
            {/snippet}
          </Select.Item>
        {/each}
      </Select.Viewport>
    </Select.Content>
  </Select.Portal>
</Select.Root>

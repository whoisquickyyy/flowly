<script lang="ts">
  import { Slider } from 'bits-ui';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    orientation?: 'horizontal' | 'vertical';
    ariaLabel?: string;
    onValueChange: (val: number) => void;
  }

  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    disabled = false,
    orientation = 'horizontal',
    ariaLabel = 'Slider',
    onValueChange,
  }: Props = $props();
</script>

<Slider.Root
  type="single"
  {value}
  {min}
  {max}
  {step}
  {disabled}
  {orientation}
  aria-label={ariaLabel}
  onValueChange={onValueChange}
  class={`rx-slider-root ${orientation === 'vertical' ? 'is-vertical' : 'is-horizontal'}`}
>
  {#snippet children({ thumbs })}
    <span class="rx-slider-track">
      <Slider.Range class="rx-slider-range" />
    </span>
    {#each thumbs as index (index)}
      <Slider.Thumb {index} class="rx-slider-thumb" />
    {/each}
  {/snippet}
</Slider.Root>

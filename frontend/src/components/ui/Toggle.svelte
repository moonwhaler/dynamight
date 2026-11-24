<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    checked = $bindable(false),
    label,
    description,
    disabled = false,
    onchange,
    extra,
    size = 'md',
  }: {
    checked?: boolean;
    /** Visible label; without one, pass `ariaLabel` semantics via `label` anyway */
    label: string;
    description?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    /** Content next to the label (a help tooltip) */
    extra?: Snippet;
    size?: 'sm' | 'md';
  } = $props();

  const id = `tg-${Math.random().toString(36).slice(2, 9)}`;

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<div class="flex items-start justify-between gap-4 {disabled ? 'opacity-60' : ''}">
  <div class="min-w-0">
    <div class="flex items-center gap-1">
      <label for={id} id="{id}-label" class="text-sm font-medium text-gray-900 dark:text-gray-100 {disabled ? '' : 'cursor-pointer'}">
        {label}
      </label>
      {#if extra}{@render extra()}{/if}
    </div>
    {#if description}
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{description}</p>
    {/if}
  </div>
  <button
    {id}
    type="button"
    role="switch"
    aria-labelledby="{id}-label"
    aria-checked={checked}
    {disabled}
    onclick={toggle}
    class="relative shrink-0 inline-flex items-center rounded-full transition-colors
      {size === 'sm' ? 'h-5 w-9' : 'h-6 w-11'}
      {checked ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-600'}
      disabled:cursor-not-allowed"
  >
    <span
      class="inline-block rounded-full bg-white shadow-sm transition-transform
        {size === 'sm' ? 'h-4 w-4' : 'h-5 w-5'}
        {checked ? (size === 'sm' ? 'translate-x-[18px]' : 'translate-x-[22px]') : 'translate-x-0.5'}"
    ></span>
  </button>
</div>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    icon = 'folder',
    title,
    text,
    action,
    compact = false,
  }: { icon?: IconName; title: string; text?: string; action?: Snippet; compact?: boolean } = $props();
</script>

<div class="flex flex-col items-center text-center {compact ? 'py-8' : 'py-14'} px-6">
  <div class="w-11 h-11 rounded-xl bg-gray-100 dark:bg-gray-700/60 text-gray-500 dark:text-gray-400 flex items-center justify-center mb-4">
    <Icon name={icon} class="w-6 h-6" />
  </div>
  <p class="font-medium text-gray-900 dark:text-white">{title}</p>
  {#if text}<p class="text-sm text-gray-500 dark:text-gray-400 mt-1 max-w-sm">{text}</p>{/if}
  {#if action}<div class="mt-5">{@render action()}</div>{/if}
</div>

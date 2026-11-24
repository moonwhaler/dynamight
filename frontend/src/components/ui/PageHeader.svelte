<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    eyebrow,
    subtitle,
    actions,
    back,
  }: {
    title: string;
    eyebrow?: string;
    subtitle?: Snippet | string;
    actions?: Snippet;
    /** Link target for a back arrow (phone layout shows it in the top bar) */
    back?: Snippet;
  } = $props();

  $effect(() => {
    document.title = `${title} · Dynamight`;
  });
</script>

<header class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between mb-6 lg:mb-8">
  <div class="min-w-0">
    {#if back}<div class="mb-2">{@render back()}</div>{/if}
    {#if eyebrow}<p class="eyebrow mb-1.5">{eyebrow}</p>{/if}
    <h1 class="font-display text-2xl sm:text-[1.75rem] leading-tight text-gray-900 dark:text-white break-words">{title}</h1>
    {#if subtitle}
      <div class="text-sm text-gray-500 dark:text-gray-400 mt-1.5">
        {#if typeof subtitle === 'string'}{subtitle}{:else}{@render subtitle()}{/if}
      </div>
    {/if}
  </div>
  {#if actions}
    <div class="flex items-center gap-2 shrink-0 flex-wrap">{@render actions()}</div>
  {/if}
</header>

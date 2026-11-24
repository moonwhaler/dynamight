<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    id,
    title,
    description,
    children,
  }: { id: string; title: string; description?: string; children: Snippet } = $props();
</script>

<section {id} class="card p-5 sm:p-6 scroll-mt-24" aria-labelledby="{id}-title">
  <h2 id="{id}-title" class="text-base font-semibold text-gray-900 dark:text-white">{title}</h2>
  {#if description}<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{description}</p>{/if}
  <div class="mt-5">{@render children()}</div>
</section>

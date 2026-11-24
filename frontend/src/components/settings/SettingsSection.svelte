<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description,
    actions,
    children,
  }: { title: string; description?: string; actions?: Snippet; children: Snippet } = $props();
</script>

<section class="card p-5 sm:p-6">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <h2 class="text-base font-semibold text-gray-900 dark:text-white">{title}</h2>
      {#if description}<p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{description}</p>{/if}
    </div>
    {#if actions}<div class="shrink-0">{@render actions()}</div>{/if}
  </div>
  <div class="mt-5">
    {@render children()}
  </div>
</section>

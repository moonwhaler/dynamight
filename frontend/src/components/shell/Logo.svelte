<script lang="ts">
  // The custom wordmark (/logo.png) when present, else a plain mark + name.
  let { compact = false, large = false }: { compact?: boolean; large?: boolean } = $props();
  let loaded = $state(false);
  let failed = $state(false);
</script>

{#if !compact && !failed}
  <img
    src="/logo.png"
    alt="Dynamight"
    class="{large ? 'h-11' : 'h-7'} w-auto object-contain {loaded ? '' : 'hidden'}"
    onload={() => (loaded = true)}
    onerror={() => (failed = true)}
  />
{/if}
{#if compact || !loaded}
  <span class="flex items-center gap-2.5">
    <span class="w-8 h-8 rounded-lg bg-primary-600 text-white flex items-center justify-center shrink-0" aria-hidden="true">
      <svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 5.625c0 2.278-3.694 4.125-8.25 4.125S3.75 14.278 3.75 12" />
      </svg>
    </span>
    {#if !compact}<span class="font-display text-lg text-gray-900 dark:text-white">Dynamight</span>{/if}
    {#if compact}<span class="sr-only">Dynamight</span>{/if}
  </span>
{/if}

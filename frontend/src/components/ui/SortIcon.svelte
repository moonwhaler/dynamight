<script lang="ts">
  interface Props {
    active: boolean;
    order: 'asc' | 'desc';
  }

  let { active, order }: Props = $props();
</script>

{#if active && order === 'asc'}
  <svg class="w-3.5 h-3.5 text-primary-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M12 19V5M5 12l7-7 7 7" />
  </svg>
{:else if active && order === 'desc'}
  <svg class="w-3.5 h-3.5 text-primary-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M12 5v14M5 12l7 7 7-7" />
  </svg>
{:else}
  <svg class="w-3.5 h-3.5 text-gray-300 dark:text-gray-600 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M8 9l4-4 4 4M16 15l-4 4-4-4" />
  </svg>
{/if}

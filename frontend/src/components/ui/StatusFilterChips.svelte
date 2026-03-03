<script lang="ts">
  import { formatStatus } from '$lib/i18n/status';

  let {
    activeStatuses,
    onToggle,
  }: {
    activeStatuses: Set<string>;
    onToggle: (status: string) => void;
  } = $props();

  const allStatuses = ['completed', 'running', 'failed', 'cancelled', 'pending'] as const;

  function chipClass(status: string): string {
    const active = activeStatuses.has(status);
    const base = 'px-3 py-1.5 rounded-full text-sm font-medium transition-all duration-200';
    if (active) {
      if (status === 'completed') return `${base} bg-green-600 text-white ring-2 ring-green-600 ring-offset-2 dark:ring-offset-gray-800`;
      if (status === 'running')   return `${base} bg-blue-600 text-white ring-2 ring-blue-600 ring-offset-2 dark:ring-offset-gray-800`;
      if (status === 'failed')    return `${base} bg-red-600 text-white ring-2 ring-red-600 ring-offset-2 dark:ring-offset-gray-800`;
      if (status === 'cancelled') return `${base} bg-yellow-600 text-white ring-2 ring-yellow-600 ring-offset-2 dark:ring-offset-gray-800`;
      return `${base} bg-gray-600 text-white ring-2 ring-gray-600 ring-offset-2 dark:ring-offset-gray-800`;
    }
    if (status === 'completed') return `${base} bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50`;
    if (status === 'running')   return `${base} bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50`;
    if (status === 'failed')    return `${base} bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50`;
    if (status === 'cancelled') return `${base} bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:hover:bg-yellow-900/50`;
    return `${base} bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600`;
  }
</script>

<div class="flex flex-wrap gap-2">
  {#each allStatuses as status}
    <button onclick={() => onToggle(status)} class={chipClass(status)}>
      {formatStatus(status)}
    </button>
  {/each}
</div>

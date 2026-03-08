<script lang="ts">
  import { tablePreferencesStore, ALL_COLUMNS, FIXED_COLUMNS, DEFAULT_VISIBLE } from '../../lib/stores/tablePreferences';
  import type { ColumnKey } from '../../lib/stores/tablePreferences';
  import * as m from '$lib/paraglide/messages.js';

  let open = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);
  let popoverEl = $state<HTMLDivElement | null>(null);

  // Only optional (non-fixed) columns shown in the selector
  const optionalColumns = ALL_COLUMNS.filter((c) => !FIXED_COLUMNS.includes(c));

  // Show indicator dot if current config differs from defaults
  const isNonDefault = $derived(
    JSON.stringify($tablePreferencesStore.visibleColumns) !== JSON.stringify(DEFAULT_VISIBLE)
  );

  function columnLabel(col: ColumnKey): string {
    switch (col) {
      case 'status': return m.history_table_status();
      case 'sources': return m.job_sources();
      case 'destination': return m.job_destination();
      case 'last_run': return m.job_last_run();
      case 'schedule': return m.jobs_col_schedule();
      case 'options': return m.job_options();
      default: return col;
    }
  }

  function toggleColumn(col: ColumnKey) {
    const isVisible = $tablePreferencesStore.visibleColumns.includes(col);
    tablePreferencesStore.setColumnVisibility(col, !isVisible);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }

  function handleClickOutside(e: MouseEvent) {
    if (!open) return;
    if (
      popoverEl && !popoverEl.contains(e.target as Node) &&
      buttonEl && !buttonEl.contains(e.target as Node)
    ) {
      open = false;
    }
  }

  $effect(() => {
    if (open) {
      document.addEventListener('keydown', handleKeydown);
      document.addEventListener('mousedown', handleClickOutside);
    } else {
      document.removeEventListener('keydown', handleKeydown);
      document.removeEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('keydown', handleKeydown);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  });
</script>

<div class="relative">
  <button
    bind:this={buttonEl}
    onclick={() => open = !open}
    class="btn btn-secondary p-2 relative"
    title={m.jobs_columns()}
    aria-label={m.jobs_columns()}
  >
    <!-- Columns icon: table outline with two vertical dividers -->
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <line x1="9" y1="3" x2="9" y2="21" />
      <line x1="15" y1="3" x2="15" y2="21" />
    </svg>
    {#if isNonDefault}
      <span class="absolute top-1 right-1 w-1.5 h-1.5 rounded-full bg-primary-500"></span>
    {/if}
  </button>

  {#if open}
    <div
      bind:this={popoverEl}
      class="absolute right-0 top-full mt-1 z-50 card shadow-lg w-56"
      role="dialog"
      aria-label={m.jobs_columns()}
    >
      <div class="p-2 space-y-0.5">
        {#each optionalColumns as col (col)}
          {@const isVisible = $tablePreferencesStore.visibleColumns.includes(col)}
          <div class="flex items-center gap-1 px-1 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700">
            <label class="flex items-center gap-2 flex-1 cursor-pointer min-w-0">
              <input
                type="checkbox"
                checked={isVisible}
                onchange={() => toggleColumn(col)}
                class="rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500 shrink-0"
              />
              <span class="text-sm text-gray-700 dark:text-gray-300 truncate">{columnLabel(col)}</span>
            </label>
          </div>
        {/each}
      </div>
      <div class="border-t border-gray-200 dark:border-gray-700 px-3 py-2">
        <button
          onclick={() => { tablePreferencesStore.reset(); }}
          class="text-xs text-gray-500 hover:text-primary-600 dark:text-gray-400 dark:hover:text-primary-400 transition-colors"
        >
          {m.jobs_columns_reset()}
        </button>
      </div>
    </div>
  {/if}
</div>

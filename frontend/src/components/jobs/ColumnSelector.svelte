<script lang="ts">
  import { tablePreferencesStore, ALL_COLUMNS, FIXED_COLUMNS, DEFAULT_VISIBLE } from '../../lib/stores/tablePreferences';
  import type { ColumnKey } from '../../lib/stores/tablePreferences';
  import * as m from '$lib/paraglide/messages.js';

  let open = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);
  let popoverEl = $state<HTMLDivElement | null>(null);

  // Only optional (non-fixed) columns shown in the selector
  const optionalColumns = ALL_COLUMNS.filter((c) => !FIXED_COLUMNS.includes(c));

  // Ordered optional columns that are currently visible
  const visibleOptional = $derived(
    $tablePreferencesStore.visibleColumns.filter((c) => !FIXED_COLUMNS.includes(c))
  );

  // All optional columns in display order (visible first in their current order, then hidden ones)
  const orderedOptional = $derived(() => {
    const visible = visibleOptional;
    const hidden = optionalColumns.filter((c) => !visible.includes(c));
    return [...visible, ...hidden];
  });

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

  function moveUp(col: ColumnKey) {
    const current = visibleOptional;
    const idx = current.indexOf(col);
    if (idx <= 0) return;
    const next = [...current];
    [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
    tablePreferencesStore.setColumnOrder(['job', ...next, 'actions']);
  }

  function moveDown(col: ColumnKey) {
    const current = visibleOptional;
    const idx = current.indexOf(col);
    if (idx < 0 || idx >= current.length - 1) return;
    const next = [...current];
    [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
    tablePreferencesStore.setColumnOrder(['job', ...next, 'actions']);
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
    class="btn btn-secondary flex items-center gap-1.5 relative"
    title={m.jobs_columns()}
  >
    <!-- Column adjust icon -->
    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
      <path stroke-linecap="round" stroke-linejoin="round" d="M9 4H5a1 1 0 00-1 1v14a1 1 0 001 1h4M9 4h6M9 4v16m6-16h4a1 1 0 011 1v14a1 1 0 01-1 1h-4M15 4v16" />
    </svg>
    <span class="hidden xl:inline">{m.jobs_columns()}</span>
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
        {#each orderedOptional() as col (col)}
          {@const isVisible = $tablePreferencesStore.visibleColumns.includes(col)}
          {@const visIdx = visibleOptional.indexOf(col)}
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
            {#if isVisible}
              <div class="flex flex-col shrink-0">
                <button
                  onclick={() => moveUp(col)}
                  disabled={visIdx <= 0}
                  class="p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 disabled:opacity-30 disabled:cursor-not-allowed"
                  title="Move up"
                >
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7" />
                  </svg>
                </button>
                <button
                  onclick={() => moveDown(col)}
                  disabled={visIdx >= visibleOptional.length - 1}
                  class="p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 disabled:opacity-30 disabled:cursor-not-allowed"
                  title="Move down"
                >
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>
              </div>
            {/if}
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

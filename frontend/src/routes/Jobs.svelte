<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { viewPreferencesStore } from '../lib/stores/viewPreferences';
  import { tablePreferencesStore, FIXED_COLUMNS, jobsSortStore } from '../lib/stores/tablePreferences';
  import type { ColumnKey, JobsSortColumn } from '../lib/stores/tablePreferences';
  import SortIcon from '../components/ui/SortIcon.svelte';
  import { statusStore } from '../lib/stores/logs';
  import JobCard from '../components/jobs/JobCard.svelte';
  import JobListRow from '../components/jobs/JobListRow.svelte';
  import ColumnSelector from '../components/jobs/ColumnSelector.svelte';
  import StatusFilterChips from '../components/ui/StatusFilterChips.svelte';
  import RunLogModal from '../components/logs/RunLogModal.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { formatStatus } from '$lib/i18n/status';

  let searchQuery = $state('');
  let statusFilters = $state<Set<string>>(new Set());
  let enabledFilter = $state<'all' | 'enabled' | 'disabled'>('all');
  let showFilters = $state(false);

  let activeRunId = $state<number | null>(null);
  let activeJobId = $state<number | null>(null);

  let resizing: { col: ColumnKey; startX: number; startWidth: number } | null = null;

  let tableContainerEl = $state<HTMLElement | null>(null);
  let containerWidth = $state(0);

  let dragCol = $state<ColumnKey | null>(null);
  let dragOverCol = $state<ColumnKey | null>(null);

  const filteredJobs = $derived.by(() => {
    let result = $jobsStore.jobs;

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      result = result.filter((job) =>
        job.name.toLowerCase().includes(query) ||
        (job.description && job.description.toLowerCase().includes(query))
      );
    }

    if (statusFilters.size > 0) {
      result = result.filter((job) => job.last_run_status && statusFilters.has(job.last_run_status));
    }

    if (enabledFilter === 'enabled') {
      result = result.filter((job) => job.enabled);
    } else if (enabledFilter === 'disabled') {
      result = result.filter((job) => !job.enabled);
    }

    return result;
  });

  const SORTABLE_JOB_COLS = new Set<ColumnKey>(['job', 'status', 'sources', 'destination', 'last_run']);

  const sortedFilteredJobs = $derived.by(() => {
    const { sortBy, sortOrder } = $jobsSortStore;
    return [...filteredJobs].sort((a, b) => {
      let cmp = 0;
      switch (sortBy) {
        case 'job':         cmp = a.name.localeCompare(b.name); break;
        case 'status':      cmp = (a.enabled ? 1 : 0) - (b.enabled ? 1 : 0); break;
        case 'sources':     cmp = (a.source_dirs?.length ?? 0) - (b.source_dirs?.length ?? 0); break;
        case 'destination': cmp = a.destination_type.localeCompare(b.destination_type); break;
        case 'last_run':    cmp = new Date(a.last_run_at ?? 0).getTime() - new Date(b.last_run_at ?? 0).getTime(); break;
      }
      return sortOrder === 'asc' ? cmp : -cmp;
    });
  });

  const activeFilterCount = $derived(
    (searchQuery.trim() ? 1 : 0) +
    (statusFilters.size > 0 ? 1 : 0) +
    (enabledFilter !== 'all' ? 1 : 0)
  );

  const storedTotalWidth = $derived(
    $tablePreferencesStore.visibleColumns.reduce(
      (sum, col) => sum + $tablePreferencesStore.columnWidths[col],
      0
    )
  );

  const effectiveWidths = $derived.by<Record<ColumnKey, number>>(() => {
    const cols = $tablePreferencesStore.visibleColumns;
    const stored = $tablePreferencesStore.columnWidths;
    const extra = containerWidth - storedTotalWidth;
    if (extra <= 0 || storedTotalWidth === 0) return { ...stored };
    const result = { ...stored };
    for (const col of cols) {
      result[col] = Math.round(stored[col] + extra * (stored[col] / storedTotalWidth));
    }
    return result;
  });

  function columnLabel(col: ColumnKey): string {
    switch (col) {
      case 'job':         return m.history_table_job();
      case 'status':      return m.history_table_status();
      case 'sources':     return m.job_sources();
      case 'destination': return m.job_destination();
      case 'last_run':    return m.job_last_run();
      case 'schedule':    return m.jobs_col_schedule();
      case 'options':     return m.job_options();
      case 'actions':     return m.common_actions();
      default:            return col;
    }
  }

  function toggleStatus(status: string) {
    const newFilters = new Set(statusFilters);
    if (newFilters.has(status)) {
      newFilters.delete(status);
    } else {
      newFilters.add(status);
    }
    statusFilters = newFilters;
  }

  function clearAllFilters() {
    searchQuery = '';
    statusFilters = new Set();
    enabledFilter = 'all';
  }

  function startResize(col: ColumnKey, e: MouseEvent) {
    e.preventDefault();
    resizing = { col, startX: e.clientX, startWidth: effectiveWidths[col] };
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', onResizeMove);
    window.addEventListener('mouseup', onResizeEnd);
  }

  function onResizeMove(e: MouseEvent) {
    if (!resizing) return;
    tablePreferencesStore.setColumnWidth(resizing.col, resizing.startWidth + (e.clientX - resizing.startX));
  }

  function onResizeEnd() {
    resizing = null;
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
  }

  let unsubscribeStatus: (() => void) | null = null;
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  const POLL_INTERVAL_MS = 3000;

  const hasRunningJobs = $derived($jobsStore.jobs.some((j) => j.last_run_status === 'running'));

  function startPolling() {
    if (pollInterval) return;
    pollInterval = setInterval(() => { jobsStore.refresh(); }, POLL_INTERVAL_MS);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  $effect(() => {
    if (hasRunningJobs) {
      startPolling();
    } else {
      stopPolling();
    }
  });

  $effect(() => {
    if (!tableContainerEl) return;
    const ro = new ResizeObserver((entries) => { containerWidth = entries[0].contentRect.width; });
    ro.observe(tableContainerEl);
    return () => ro.disconnect();
  });

  function onDragStart(col: ColumnKey, e: DragEvent) {
    dragCol = col;
    e.dataTransfer!.effectAllowed = 'move';
  }

  function onDragOver(col: ColumnKey, e: DragEvent) {
    if (!dragCol || dragCol === col || FIXED_COLUMNS.includes(col)) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
    dragOverCol = col;
  }

  function onDrop(col: ColumnKey, e: DragEvent) {
    e.preventDefault();
    if (!dragCol || dragCol === col || FIXED_COLUMNS.includes(col)) return;
    const cols = [...$tablePreferencesStore.visibleColumns];
    const fromIdx = cols.indexOf(dragCol);
    const toIdx = cols.indexOf(col);
    if (fromIdx < 0 || toIdx < 0) return;
    cols.splice(fromIdx, 1);
    cols.splice(fromIdx < toIdx ? toIdx - 1 : toIdx, 0, dragCol);
    tablePreferencesStore.setColumnOrder(cols);
    dragCol = null;
    dragOverCol = null;
  }

  function onDragEnd() {
    dragCol = null;
    dragOverCol = null;
  }

  onMount(() => {
    jobsStore.load();
    statusStore.connect();
    unsubscribeStatus = statusStore.subscribe(() => { jobsStore.refresh(); });
    return () => { statusStore.disconnect(); };
  });

  onDestroy(() => {
    stopPolling();
    if (unsubscribeStatus) unsubscribeStatus();
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
    document.body.style.userSelect = '';
  });
</script>

{#snippet viewToggle()}
  <div class="flex items-center border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
    <button
      onclick={() => viewPreferencesStore.setViewMode('grid')}
      class="py-2.5 px-2 transition-colors {$viewPreferencesStore === 'grid'
        ? 'bg-primary-600 text-white'
        : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
      title={m.jobs_grid_view()}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
      </svg>
    </button>
    <button
      onclick={() => viewPreferencesStore.setViewMode('list')}
      class="py-2.5 px-2 transition-colors {$viewPreferencesStore === 'list'
        ? 'bg-primary-600 text-white'
        : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
      title={m.jobs_list_view()}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
      </svg>
    </button>
  </div>
{/snippet}

<div class="space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{m.jobs_title()}</h1>
    <div class="flex items-center gap-2">
      {@render viewToggle()}
      {#if $viewPreferencesStore === 'list'}
        <ColumnSelector />
      {/if}
      <a href="#/jobs/new" class="btn btn-primary sm:w-auto text-center">{m.dashboard_new_job()}</a>
    </div>
  </div>

  {#if $jobsStore.loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else if $jobsStore.error}
    <div class="card p-8 text-center">
      <p class="text-red-600 dark:text-red-400 mb-4">{$jobsStore.error}</p>
      <button onclick={() => jobsStore.load()} class="btn btn-secondary">{m.common_retry()}</button>
    </div>
  {:else if $jobsStore.jobs.length === 0}
    <div class="card p-12 text-center">
      <svg class="mx-auto h-16 w-16 text-gray-400 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{m.jobs_no_jobs()}</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-6">{m.jobs_no_jobs_description()}</p>
      <a href="#/jobs/new" class="btn btn-primary">{m.job_create()}</a>
    </div>
  {:else}
    <div class="card">
      <div class="p-4">
        <div class="flex flex-col lg:flex-row gap-4">
          <div class="relative flex-1 min-w-0">
            <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input type="text" bind:value={searchQuery} placeholder={m.jobs_search_placeholder()} class="input pl-10" />
          </div>

          <select bind:value={enabledFilter} class="input lg:w-40">
            <option value="all">{m.jobs_filter_all()}</option>
            <option value="enabled">{m.jobs_filter_active()}</option>
            <option value="disabled">{m.jobs_filter_disabled()}</option>
          </select>

          <button
            onclick={() => showFilters = !showFilters}
            class="btn btn-secondary flex items-center justify-center gap-2 lg:hidden"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
            </svg>
            {m.common_filters()}
            {#if activeFilterCount > 0}
              <span class="bg-primary-600 text-white text-xs font-medium px-2 py-0.5 rounded-full">{activeFilterCount}</span>
            {/if}
          </button>

        </div>

        <!-- Desktop: Status filter chips -->
        <div class="hidden lg:flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
          <span class="text-sm text-gray-500 dark:text-gray-400">{m.jobs_last_run_label()}</span>
          <StatusFilterChips activeStatuses={statusFilters} onToggle={toggleStatus} />
          {#if activeFilterCount > 0}
            <button
              onclick={clearAllFilters}
              class="ml-auto text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 flex items-center gap-1"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
              {m.jobs_clear_filters()}
            </button>
          {/if}
        </div>

        <!-- Mobile/Tablet: Expanded filters -->
        {#if showFilters}
          <div class="lg:hidden mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 space-y-4">
            <div class="space-y-2">
              <span class="label">{m.jobs_last_run_status()}</span>
              <StatusFilterChips activeStatuses={statusFilters} onToggle={toggleStatus} />
            </div>

            {#if activeFilterCount > 0}
              <button onclick={clearAllFilters} class="btn btn-sm btn-secondary w-full">
                {m.jobs_clear_all_filters()}
              </button>
            {/if}
          </div>
        {/if}
      </div>

      {#if filteredJobs.length !== $jobsStore.jobs.length}
        <div class="px-4 py-2 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 rounded-b-xl">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {m.jobs_showing_filtered({ shown: filteredJobs.length, total: $jobsStore.jobs.length })}
          </p>
        </div>
      {/if}
    </div>

    {#if filteredJobs.length === 0}
      <div class="card p-12 text-center">
        <svg class="mx-auto h-16 w-16 text-gray-400 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
        </svg>
        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{m.jobs_no_matching()}</h3>
        <p class="text-gray-500 dark:text-gray-400 mb-4">{m.jobs_no_matching_description()}</p>
        <button onclick={clearAllFilters} class="btn btn-secondary">{m.jobs_clear_all_filters()}</button>
      </div>
    {:else if $viewPreferencesStore === 'grid'}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each sortedFilteredJobs as job (job.id)}
          <JobCard {job} />
        {/each}
      </div>
    {:else}
      <div class="card overflow-hidden">
        <div class="overflow-x-auto" bind:this={tableContainerEl}>
          <table
            class="min-w-full table-fixed divide-y divide-gray-200 dark:divide-gray-700"
            style="width: {Math.max(storedTotalWidth, containerWidth)}px"
          >
            <colgroup>
              {#each $tablePreferencesStore.visibleColumns as col (col)}
                <col style="width: {effectiveWidths[col]}px" />
              {/each}
            </colgroup>
            <thead class="bg-gray-50 dark:bg-gray-800/50">
              <tr>
                {#each $tablePreferencesStore.visibleColumns as col (col)}
                  <th
                    class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase relative select-none
                      {!FIXED_COLUMNS.includes(col) ? 'cursor-grab' : ''}
                      {dragOverCol === col ? 'bg-primary-50 dark:bg-primary-900/20' : ''}"
                    style="width: {effectiveWidths[col]}px"
                    draggable={!FIXED_COLUMNS.includes(col)}
                    ondragstart={!FIXED_COLUMNS.includes(col) ? (e) => onDragStart(col, e) : undefined}
                    ondragover={(e) => onDragOver(col, e)}
                    ondragleave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node) && dragOverCol === col) dragOverCol = null; }}
                    ondrop={(e) => onDrop(col, e)}
                    ondragend={onDragEnd}
                  >
                    {#if SORTABLE_JOB_COLS.has(col)}
                      <button
                        type="button"
                        onclick={() => jobsSortStore.handleSort(col as JobsSortColumn)}
                        class="flex items-center gap-1 hover:text-gray-700 dark:hover:text-gray-200 cursor-pointer"
                      >
                        {columnLabel(col)}
                        <SortIcon active={$jobsSortStore.sortBy === col} order={$jobsSortStore.sortOrder} />
                      </button>
                    {:else}
                      {columnLabel(col)}
                    {/if}
                    {#if dragOverCol === col}
                      <div class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary-500 pointer-events-none"></div>
                    {/if}
                    {#if col !== 'actions'}
                      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                      <div
                        class="absolute right-0 top-0 bottom-0 w-3 translate-x-1/2 z-10 cursor-col-resize group"
                        onmousedown={(e) => startResize(col, e)}
                        ondragstart={(e) => e.stopPropagation()}
                        role="separator"
                        aria-orientation="vertical"
                      >
                        <div class="absolute left-1/2 -translate-x-1/2 top-0 bottom-0 w-px bg-gray-200 dark:bg-gray-700 group-hover:bg-primary-500 dark:group-hover:bg-primary-400 transition-colors duration-150"></div>
                      </div>
                    {/if}
                  </th>
                {/each}
              </tr>
            </thead>
            <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {#each sortedFilteredJobs as job (job.id)}
                <JobListRow
                  {job}
                  onShowLogs={(runId) => { activeRunId = runId; activeJobId = job.id; }}
                />
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  {/if}
</div>

{#if activeRunId !== null && activeJobId !== null}
  <RunLogModal
    runId={activeRunId}
    jobId={activeJobId}
    onClose={() => { activeRunId = null; activeJobId = null; }}
  />
{/if}

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import { historyTablePreferencesStore, HISTORY_FIXED, HISTORY_ALL, HISTORY_DEFAULT_VISIBLE, historySortStore } from '../lib/stores/historyTablePreferences';
  import type { HistoryColumnKey, HistorySortColumn } from '../lib/stores/historyTablePreferences';
  import SortIcon from '../components/ui/SortIcon.svelte';
  import type { Job, JobRun, LogEntry } from '../lib/types';
  import LogViewer from '../components/logs/LogViewer.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import StatusFilterChips from '../components/ui/StatusFilterChips.svelte';
  import ColumnSelector from '../components/ui/ColumnSelector.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { formatStatus, getStatusBadgeClass } from '$lib/i18n/status';
  import { formatDateTime, formatDurationBetween } from '$lib/i18n/relativeTime';
  import { formatBytes } from '$lib/utils/format';

  let runs = $state<JobRun[]>([]);
  let loading = $state(true);
  let selectedJobId = $state<number | null>(null);
  let selectedRun = $state<JobRun | null>(null);
  let logs = $state<LogEntry[]>([]);
  let logsTotal = $state(0);
  let logsCurrentPage = $state(1);
  let loadingLogs = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  let searchQuery = $state('');
  let statusFilters = $state<Set<string>>(new Set());
  let dateFrom = $state<string>('');
  let dateTo = $state<string>('');
  let showFilters = $state(false);

  const LOG_PAGE_SIZE = 500;
  const POLL_INTERVAL_MS = 3000;

  const logsTotalPages = $derived(Math.max(1, Math.ceil(logsTotal / LOG_PAGE_SIZE)));
  const hasRunningJobs = $derived(runs.some((r) => r.status === 'running'));

  const filteredRuns = $derived.by(() => {
    let result = runs;

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      result = result.filter((run) => getJobName(run.job_id).toLowerCase().includes(query));
    }

    if (statusFilters.size > 0) {
      result = result.filter((run) => statusFilters.has(run.status));
    }

    if (dateFrom) {
      const fromDate = new Date(dateFrom);
      fromDate.setHours(0, 0, 0, 0);
      result = result.filter((run) => run.started_at && new Date(run.started_at) >= fromDate);
    }

    if (dateTo) {
      const toDate = new Date(dateTo);
      toDate.setHours(23, 59, 59, 999);
      result = result.filter((run) => run.started_at && new Date(run.started_at) <= toDate);
    }

    return result;
  });

  const SORTABLE_HISTORY_COLS = new Set<HistoryColumnKey>(['job', 'status', 'started', 'duration', 'files', 'size']);

  const sortedFilteredRuns = $derived.by(() => {
    const { sortBy, sortOrder } = $historySortStore;
    const STATUS_ORDER: Record<string, number> = { pending: 0, running: 1, completed: 2, failed: 3, cancelled: 4 };
    return [...filteredRuns].sort((a, b) => {
      let cmp = 0;
      switch (sortBy) {
        case 'job':      cmp = getJobName(a.job_id).localeCompare(getJobName(b.job_id)); break;
        case 'status':   cmp = (STATUS_ORDER[a.status] ?? 0) - (STATUS_ORDER[b.status] ?? 0); break;
        case 'started':  cmp = new Date(a.started_at ?? 0).getTime() - new Date(b.started_at ?? 0).getTime(); break;
        case 'duration': {
          const durA = a.completed_at && a.started_at ? new Date(a.completed_at).getTime() - new Date(a.started_at).getTime() : 0;
          const durB = b.completed_at && b.started_at ? new Date(b.completed_at).getTime() - new Date(b.started_at).getTime() : 0;
          cmp = durA - durB; break;
        }
        case 'files':    cmp = (a.files_transferred ?? 0) - (b.files_transferred ?? 0); break;
        case 'size':     cmp = (a.bytes_transferred ?? 0) - (b.bytes_transferred ?? 0); break;
      }
      return sortOrder === 'asc' ? cmp : -cmp;
    });
  });

  const activeFilterCount = $derived(
    (searchQuery.trim() ? 1 : 0) +
    (statusFilters.size > 0 ? 1 : 0) +
    (dateFrom ? 1 : 0) +
    (dateTo ? 1 : 0) +
    (selectedJobId ? 1 : 0)
  );

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
    dateFrom = '';
    dateTo = '';
    selectedJobId = null;
  }

  onMount(async () => {
    await jobsStore.load();
    await loadRuns();
  });

  onDestroy(() => {
    stopPolling();
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
    document.body.style.userSelect = '';
  });

  function startPolling() {
    if (pollInterval) return;
    pollInterval = setInterval(async () => {
      await loadRuns(true);
    }, POLL_INTERVAL_MS);
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

  async function loadRuns(silent = false) {
    if (!silent) loading = true;
    try {
      const allRuns: JobRun[] = [];
      const jobs = selectedJobId
        ? $jobsStore.jobs.filter((j) => j.id === selectedJobId)
        : $jobsStore.jobs;

      for (const job of jobs) {
        const jobRuns = await api.runs.list(job.id, 20);
        allRuns.push(...jobRuns);
      }

      runs = allRuns.sort(
        (a, b) => new Date(b.started_at || 0).getTime() - new Date(a.started_at || 0).getTime()
      );
    } catch {
      // Ignore
    } finally {
      if (!silent) loading = false;
    }
  }

  async function selectRun(run: JobRun) {
    selectedRun = run;
    logsCurrentPage = 1;
    await loadLogsPage(1);
  }

  async function loadLogsPage(page: number) {
    if (!selectedRun) return;
    loadingLogs = true;
    try {
      const offset = (page - 1) * LOG_PAGE_SIZE;
      const response = await api.runs.logs(selectedRun.id, LOG_PAGE_SIZE, offset);
      logs = response.entries;
      logsTotal = response.total;
      logsCurrentPage = page;
    } catch {
      logs = [];
    } finally {
      loadingLogs = false;
    }
  }

  function closeDetails() {
    selectedRun = null;
    logs = [];
    logsTotal = 0;
    logsCurrentPage = 1;
  }

  function getJobName(jobId: number): string {
    return $jobsStore.jobs.find((j) => j.id === jobId)?.name || `Job #${jobId}`;
  }

  let tableContainerEl = $state<HTMLElement | null>(null);
  let containerWidth = $state(0);
  let dragCol = $state<HistoryColumnKey | null>(null);
  let dragOverCol = $state<HistoryColumnKey | null>(null);
  let resizing: { col: HistoryColumnKey; startX: number; startWidth: number } | null = null;

  const storedTotalWidth = $derived(
    $historyTablePreferencesStore.visibleColumns.reduce(
      (sum, col) => sum + $historyTablePreferencesStore.columnWidths[col],
      0
    )
  );

  const effectiveWidths = $derived.by<Record<HistoryColumnKey, number>>(() => {
    const cols = $historyTablePreferencesStore.visibleColumns;
    const stored = $historyTablePreferencesStore.columnWidths;
    const extra = containerWidth - storedTotalWidth;
    if (extra <= 0 || storedTotalWidth === 0) return { ...stored };
    const result = { ...stored };
    for (const col of cols) {
      result[col] = Math.round(stored[col] + extra * (stored[col] / storedTotalWidth));
    }
    return result;
  });

  function columnLabel(col: string): string {
    switch (col as HistoryColumnKey) {
      case 'job':      return m.history_table_job();
      case 'status':   return m.history_table_status();
      case 'started':  return m.history_table_started();
      case 'duration': return m.history_table_duration();
      case 'files':    return m.history_table_files();
      case 'size':     return m.history_table_size();
      case 'actions':  return m.common_actions();
      default:         return col;
    }
  }

  function handleColumnToggle(col: string) {
    const key = col as HistoryColumnKey;
    historyTablePreferencesStore.setColumnVisibility(
      key,
      !$historyTablePreferencesStore.visibleColumns.includes(key)
    );
  }

  function startResize(col: HistoryColumnKey, e: MouseEvent) {
    e.preventDefault();
    resizing = { col, startX: e.clientX, startWidth: effectiveWidths[col] };
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', onResizeMove);
    window.addEventListener('mouseup', onResizeEnd);
  }

  function onResizeMove(e: MouseEvent) {
    if (!resizing) return;
    historyTablePreferencesStore.setColumnWidth(resizing.col, resizing.startWidth + (e.clientX - resizing.startX));
  }

  function onResizeEnd() {
    resizing = null;
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
  }

  function onDragStart(col: HistoryColumnKey, e: DragEvent) {
    dragCol = col;
    e.dataTransfer!.effectAllowed = 'move';
  }

  function onDragOver(col: HistoryColumnKey, e: DragEvent) {
    if (!dragCol || dragCol === col || HISTORY_FIXED.includes(col)) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
    dragOverCol = col;
  }

  function onDrop(col: HistoryColumnKey, e: DragEvent) {
    e.preventDefault();
    if (!dragCol || dragCol === col || HISTORY_FIXED.includes(col)) return;
    const cols = [...$historyTablePreferencesStore.visibleColumns];
    const fromIdx = cols.indexOf(dragCol);
    const toIdx = cols.indexOf(col);
    if (fromIdx < 0 || toIdx < 0) return;
    cols.splice(fromIdx, 1);
    cols.splice(fromIdx < toIdx ? toIdx - 1 : toIdx, 0, dragCol);
    historyTablePreferencesStore.setColumnOrder(cols);
    dragCol = null;
    dragOverCol = null;
  }

  function onDragEnd() {
    dragCol = null;
    dragOverCol = null;
  }

  $effect(() => {
    if (!tableContainerEl) return;
    const ro = new ResizeObserver((entries) => { containerWidth = entries[0].contentRect.width; });
    ro.observe(tableContainerEl);
    return () => ro.disconnect();
  });

  let showPurgeConfirm = $state(false);
  let purgeType = $state<'all' | 'job' | 'single'>('all');
  let purgeTargetId = $state<number | null>(null);
  let purging = $state(false);

  function confirmPurgeAll() {
    purgeType = 'all';
    purgeTargetId = null;
    showPurgeConfirm = true;
  }

  function confirmPurgeJob() {
    if (selectedJobId) {
      purgeType = 'job';
      purgeTargetId = selectedJobId;
      showPurgeConfirm = true;
    }
  }

  function confirmDeleteRun(runId: number) {
    purgeType = 'single';
    purgeTargetId = runId;
    showPurgeConfirm = true;
  }

  function cancelPurge() {
    showPurgeConfirm = false;
    purgeTargetId = null;
  }

  async function executePurge() {
    purging = true;
    try {
      if (purgeType === 'all') {
        await api.runs.purgeAll();
      } else if (purgeType === 'job' && purgeTargetId) {
        await api.runs.deleteForJob(purgeTargetId);
      } else if (purgeType === 'single' && purgeTargetId) {
        await api.runs.delete(purgeTargetId);
      }
      await loadRuns();
    } catch {
      // Ignore
    } finally {
      purging = false;
      showPurgeConfirm = false;
      purgeTargetId = null;
    }
  }

  function getPurgeMessage(): string {
    if (purgeType === 'all') return m.history_purge_all_confirm();
    if (purgeType === 'job' && purgeTargetId) return m.history_purge_job_confirm({ name: getJobName(purgeTargetId) });
    if (purgeType === 'single') return m.history_purge_run_confirm();
    return '';
  }
</script>

<div class="space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{m.history_title()}</h1>

    <div class="flex items-center gap-2">
      {#if runs.length > 0 && selectedJobId}
        <button onclick={confirmPurgeJob} class="btn btn-secondary">{m.history_clear_job()}</button>
      {/if}
      <button onclick={confirmPurgeAll} disabled={runs.length === 0} class="btn btn-secondary">
        {m.history_clear_all()}
      </button>
      {#if runs.length > 0}
        <ColumnSelector
          visibleColumns={$historyTablePreferencesStore.visibleColumns}
          allColumns={HISTORY_ALL}
          fixedColumns={HISTORY_FIXED}
          defaultVisible={HISTORY_DEFAULT_VISIBLE}
          columnLabel={columnLabel}
          onToggle={handleColumnToggle}
          onReset={() => historyTablePreferencesStore.reset()}
        />
      {/if}
    </div>
  </div>

  <!-- Filter Bar -->
  <div class="card">
    <div class="p-4">
      <div class="flex flex-col lg:flex-row gap-4">
        <div class="relative flex-1 min-w-0">
          <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input type="text" bind:value={searchQuery} placeholder={m.history_search_placeholder()} class="input pl-10" />
        </div>

        <select bind:value={selectedJobId} onchange={() => loadRuns()} class="input lg:w-48">
          <option value={null}>{m.jobs_filter_all()}</option>
          {#each $jobsStore.jobs as job}
            <option value={job.id}>{job.name}</option>
          {/each}
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

        <div class="hidden lg:flex items-center gap-2">
          <input type="date" bind:value={dateFrom} class="input w-40 text-sm" aria-label={m.history_filter_from()} />
          <span class="text-gray-400">-</span>
          <input type="date" bind:value={dateTo} class="input w-40 text-sm" aria-label={m.history_filter_to()} />
        </div>
      </div>

      <!-- Desktop: Status filter chips -->
      <div class="hidden lg:flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
        <span class="text-sm text-gray-500 dark:text-gray-400">{m.history_table_status()}:</span>
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
            <span class="label">{m.history_date_range()}</span>
            <div class="flex items-center gap-2">
              <input type="date" bind:value={dateFrom} class="input flex-1 text-sm" aria-label={m.history_filter_from()} />
              <span class="text-gray-400">-</span>
              <input type="date" bind:value={dateTo} class="input flex-1 text-sm" aria-label={m.history_filter_to()} />
            </div>
          </div>

          <div class="space-y-2">
            <span class="label">{m.history_table_status()}</span>
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

    {#if !loading && filteredRuns.length !== runs.length}
      <div class="px-4 py-2 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 rounded-b-xl">
        <p class="text-sm text-gray-500 dark:text-gray-400">
          {m.history_showing_count({
            shown: filteredRuns.length,
            total: runs.length,
            unit: filteredRuns.length === 1 ? m.history_run_singular() : m.history_runs_plural()
          })}
        </p>
      </div>
    {/if}
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <Spinner size="w-10 h-10" />
    </div>
  {:else if runs.length === 0}
    <div class="card p-12 text-center">
      <svg class="mx-auto h-16 w-16 text-gray-400 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{m.history_no_runs()}</h3>
      <p class="text-gray-500 dark:text-gray-400">{m.history_no_runs_description()}</p>
    </div>
  {:else if filteredRuns.length === 0}
    <div class="card p-12 text-center">
      <svg class="mx-auto h-16 w-16 text-gray-400 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{m.history_no_matching()}</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-4">{m.history_no_matching_description()}</p>
      <button onclick={clearAllFilters} class="btn btn-secondary">{m.jobs_clear_all_filters()}</button>
    </div>
  {:else}
    <div class="card overflow-hidden">
      <div class="overflow-x-auto" bind:this={tableContainerEl}>
        <table
          class="min-w-full table-fixed divide-y divide-gray-200 dark:divide-gray-700"
          style="width: {Math.max(storedTotalWidth, containerWidth)}px"
        >
          <colgroup>
            {#each $historyTablePreferencesStore.visibleColumns as col (col)}
              <col style="width: {effectiveWidths[col]}px" />
            {/each}
          </colgroup>
          <thead class="bg-gray-50 dark:bg-gray-800/50">
            <tr>
              {#each $historyTablePreferencesStore.visibleColumns as col (col)}
                <th
                  class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase relative select-none
                    {!HISTORY_FIXED.includes(col) ? 'cursor-grab' : ''}
                    {dragOverCol === col ? 'bg-primary-50 dark:bg-primary-900/20' : ''}"
                  style="width: {effectiveWidths[col]}px"
                  draggable={!HISTORY_FIXED.includes(col)}
                  ondragstart={!HISTORY_FIXED.includes(col) ? (e) => onDragStart(col, e) : undefined}
                  ondragover={(e) => onDragOver(col, e)}
                  ondragleave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node) && dragOverCol === col) dragOverCol = null; }}
                  ondrop={(e) => onDrop(col, e)}
                  ondragend={onDragEnd}
                >
                  {#if SORTABLE_HISTORY_COLS.has(col)}
                    <button
                      type="button"
                      onclick={() => historySortStore.handleSort(col as HistorySortColumn)}
                      class="flex items-center gap-1 hover:text-gray-700 dark:hover:text-gray-200 cursor-pointer"
                    >
                      {columnLabel(col)}
                      <SortIcon active={$historySortStore.sortBy === col} order={$historySortStore.sortOrder} />
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
            {#each sortedFilteredRuns as run (run.id)}
              <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                {#each $historyTablePreferencesStore.visibleColumns as col (col)}
                  {#if col === 'job'}
                    <td class="px-4 py-3 text-sm text-gray-900 dark:text-gray-100">{getJobName(run.job_id)}</td>
                  {:else if col === 'status'}
                    <td class="px-4 py-3">
                      <span class="badge {getStatusBadgeClass(run.status)}">{formatStatus(run.status)}</span>
                    </td>
                  {:else if col === 'started'}
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">{formatDateTime(run.started_at)}</td>
                  {:else if col === 'duration'}
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
                      {formatDurationBetween(run.started_at, run.completed_at)}
                    </td>
                  {:else if col === 'files'}
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{run.files_transferred ?? '-'}</td>
                  {:else if col === 'size'}
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{formatBytes(run.bytes_transferred)}</td>
                  {:else if col === 'actions'}
                    <td class="px-4 py-3">
                      <div class="flex items-center gap-2">
                        <button
                          onclick={() => selectRun(run)}
                          class="text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 p-1"
                          title={m.history_logs()}
                        >
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                          </svg>
                        </button>
                        <button
                          onclick={() => confirmDeleteRun(run.id)}
                          class="text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 text-sm p-1"
                          title={m.history_delete_this_run()}
                        >
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                        </button>
                      </div>
                    </td>
                  {/if}
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>

{#if selectedRun}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-2 sm:p-4">
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-7xl h-[95vh] sm:h-[90vh] flex flex-col">
      <div class="p-3 sm:p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between gap-2">
        <div class="min-w-0 flex-1">
          <h3 class="text-base sm:text-lg font-semibold text-gray-900 dark:text-white truncate">
            {getJobName(selectedRun.job_id)} - Run #{selectedRun.id}
          </h3>
          <p class="text-xs sm:text-sm text-gray-500 dark:text-gray-400">{formatDateTime(selectedRun.started_at)}</p>
        </div>
        <button
          onclick={closeDetails}
          class="flex-shrink-0 p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
          aria-label={m.history_close_details()}
        >
          <svg class="w-5 h-5 sm:w-6 sm:h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div class="flex-1 min-h-0 overflow-hidden relative">
        <div class="absolute inset-0">
          <LogViewer
            {logs}
            total={logsTotal}
            currentPage={logsCurrentPage}
            totalPages={logsTotalPages}
            loading={loadingLogs}
            pageSize={LOG_PAGE_SIZE}
            onPageChange={(page) => loadLogsPage(page)}
          />
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showPurgeConfirm}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-md w-full p-6">
      <div class="flex items-center gap-3 mb-4">
        <div class="w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center flex-shrink-0">
          <svg class="w-5 h-5 text-red-600 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.history_confirm_delete_title()}</h3>
      </div>
      <p class="text-gray-600 dark:text-gray-300 mb-6">{getPurgeMessage()}</p>
      <div class="flex justify-end gap-3">
        <button onclick={cancelPurge} class="btn btn-secondary" disabled={purging}>{m.common_cancel()}</button>
        <button onclick={executePurge} class="btn btn-danger" disabled={purging}>
          {#if purging}
            <Spinner />
          {/if}
          {m.common_delete()}
        </button>
      </div>
    </div>
  </div>
{/if}

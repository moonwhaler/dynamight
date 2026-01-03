<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import type { Job, JobRun, LogEntry } from '../lib/types';
  import LogViewer from '../components/logs/LogViewer.svelte';

  let runs = $state<JobRun[]>([]);
  let loading = $state(true);
  let selectedJobId = $state<number | null>(null);
  let selectedRun = $state<JobRun | null>(null);
  let logs = $state<LogEntry[]>([]);
  let logsTotal = $state(0);
  let logsCurrentPage = $state(1);
  let loadingLogs = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  // Filter state
  let searchQuery = $state('');
  let statusFilters = $state<Set<string>>(new Set());
  let dateFrom = $state<string>('');
  let dateTo = $state<string>('');
  let showFilters = $state(false);

  const allStatuses = ['completed', 'running', 'failed', 'cancelled', 'pending'] as const;

  const LOG_PAGE_SIZE = 500;
  const POLL_INTERVAL_MS = 3000;

  const logsTotalPages = $derived(Math.max(1, Math.ceil(logsTotal / LOG_PAGE_SIZE)));
  const hasRunningJobs = $derived(runs.some((r) => r.status === 'running'));

  // Filtered runs based on all filter criteria
  const filteredRuns = $derived.by(() => {
    let result = runs;

    // Filter by search query (job name)
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      result = result.filter((run) => {
        const jobName = getJobName(run.job_id).toLowerCase();
        return jobName.includes(query);
      });
    }

    // Filter by status
    if (statusFilters.size > 0) {
      result = result.filter((run) => statusFilters.has(run.status));
    }

    // Filter by date range
    if (dateFrom) {
      const fromDate = new Date(dateFrom);
      fromDate.setHours(0, 0, 0, 0);
      result = result.filter((run) => {
        if (!run.started_at) return false;
        return new Date(run.started_at) >= fromDate;
      });
    }

    if (dateTo) {
      const toDate = new Date(dateTo);
      toDate.setHours(23, 59, 59, 999);
      result = result.filter((run) => {
        if (!run.started_at) return false;
        return new Date(run.started_at) <= toDate;
      });
    }

    return result;
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

  function handlePageChange(page: number) {
    loadLogsPage(page);
  }

  function closeDetails() {
    selectedRun = null;
    logs = [];
    logsTotal = 0;
    logsCurrentPage = 1;
  }

  function formatDate(date: string | null): string {
    if (!date) return 'Never';
    return new Date(date).toLocaleString();
  }

  function formatDuration(start: string | null, end: string | null): string {
    if (!start) return '-';
    const startDate = new Date(start);
    const endDate = end ? new Date(end) : new Date();
    const diff = Math.floor((endDate.getTime() - startDate.getTime()) / 1000);
    if (diff < 60) return `${diff}s`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ${diff % 60}s`;
    return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
  }

  function formatBytes(bytes: number | null): string {
    if (bytes === null) return '-';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  function getStatusBadge(status: string): string {
    switch (status) {
      case 'completed':
        return 'badge-success';
      case 'running':
        return 'badge-info';
      case 'failed':
        return 'badge-error';
      case 'cancelled':
        return 'badge-warning';
      default:
        return 'badge-gray';
    }
  }

  function getJobName(jobId: number): string {
    return $jobsStore.jobs.find((j) => j.id === jobId)?.name || `Job #${jobId}`;
  }

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
    if (purgeType === 'all') {
      return 'Are you sure you want to delete all backup history? This action cannot be undone.';
    } else if (purgeType === 'job' && purgeTargetId) {
      const jobName = getJobName(purgeTargetId);
      return `Are you sure you want to delete all history for "${jobName}"? This action cannot be undone.`;
    } else if (purgeType === 'single' && purgeTargetId) {
      return 'Are you sure you want to delete this run? This action cannot be undone.';
    }
    return '';
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Backup History</h1>

    <div class="flex items-center gap-2">
      {#if runs.length > 0}
        {#if selectedJobId}
          <button
            onclick={confirmPurgeJob}
            class="btn btn-secondary text-sm"
          >
            Clear Job
          </button>
        {/if}
        <button
          onclick={confirmPurgeAll}
          class="btn btn-secondary text-sm"
        >
          Clear All
        </button>
      {/if}
    </div>
  </div>

  <!-- Filter Bar -->
  <div class="card">
    <div class="p-4">
      <!-- Main filter row -->
      <div class="flex flex-col lg:flex-row gap-4">
        <!-- Search input -->
        <div class="relative flex-1 min-w-0">
          <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search by job name..."
            class="input pl-10"
          />
        </div>

        <!-- Job selector -->
        <select
          bind:value={selectedJobId}
          onchange={() => loadRuns()}
          class="input lg:w-48"
        >
          <option value={null}>All Jobs</option>
          {#each $jobsStore.jobs as job}
            <option value={job.id}>{job.name}</option>
          {/each}
        </select>

        <!-- Filter toggle button (mobile/tablet) -->
        <button
          onclick={() => showFilters = !showFilters}
          class="btn btn-secondary flex items-center justify-center gap-2 lg:hidden"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
          </svg>
          Filters
          {#if activeFilterCount > 0}
            <span class="bg-primary-600 text-white text-xs font-medium px-2 py-0.5 rounded-full">{activeFilterCount}</span>
          {/if}
        </button>

        <!-- Desktop: Date range inputs -->
        <div class="hidden lg:flex items-center gap-2">
          <div class="relative">
            <input
              type="date"
              bind:value={dateFrom}
              class="input w-40 text-sm"
              aria-label="From date"
            />
            {#if !dateFrom}
              <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-sm pointer-events-none">From</span>
            {/if}
          </div>
          <span class="text-gray-400">-</span>
          <div class="relative">
            <input
              type="date"
              bind:value={dateTo}
              class="input w-40 text-sm"
              aria-label="To date"
            />
            {#if !dateTo}
              <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-sm pointer-events-none">To</span>
            {/if}
          </div>
        </div>
      </div>

      <!-- Desktop: Status filter chips -->
      <div class="hidden lg:flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
        <span class="text-sm text-gray-500 dark:text-gray-400">Status:</span>
        <div class="flex flex-wrap gap-2">
          {#each allStatuses as status}
            <button
              onclick={() => toggleStatus(status)}
              class="px-3 py-1.5 rounded-full text-sm font-medium transition-all duration-200
                {statusFilters.has(status)
                  ? status === 'completed' ? 'bg-green-600 text-white ring-2 ring-green-600 ring-offset-2 dark:ring-offset-gray-800'
                  : status === 'running' ? 'bg-blue-600 text-white ring-2 ring-blue-600 ring-offset-2 dark:ring-offset-gray-800'
                  : status === 'failed' ? 'bg-red-600 text-white ring-2 ring-red-600 ring-offset-2 dark:ring-offset-gray-800'
                  : status === 'cancelled' ? 'bg-yellow-600 text-white ring-2 ring-yellow-600 ring-offset-2 dark:ring-offset-gray-800'
                  : 'bg-gray-600 text-white ring-2 ring-gray-600 ring-offset-2 dark:ring-offset-gray-800'
                  : status === 'completed' ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50'
                  : status === 'running' ? 'bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50'
                  : status === 'failed' ? 'bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50'
                  : status === 'cancelled' ? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:hover:bg-yellow-900/50'
                  : 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600'
                }"
            >
              {status}
            </button>
          {/each}
        </div>

        {#if activeFilterCount > 0}
          <button
            onclick={clearAllFilters}
            class="ml-auto text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 flex items-center gap-1"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
            Clear filters
          </button>
        {/if}
      </div>

      <!-- Mobile/Tablet: Expanded filters -->
      {#if showFilters}
        <div class="lg:hidden mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 space-y-4">
          <!-- Date range -->
          <div class="space-y-2">
            <span class="label">Date Range</span>
            <div class="flex items-center gap-2">
              <input
                type="date"
                bind:value={dateFrom}
                class="input flex-1 text-sm"
                aria-label="From date"
              />
              <span class="text-gray-400">-</span>
              <input
                type="date"
                bind:value={dateTo}
                class="input flex-1 text-sm"
                aria-label="To date"
              />
            </div>
          </div>

          <!-- Status filters -->
          <div class="space-y-2">
            <span class="label">Status</span>
            <div class="flex flex-wrap gap-2">
              {#each allStatuses as status}
                <button
                  onclick={() => toggleStatus(status)}
                  class="px-3 py-1.5 rounded-full text-sm font-medium transition-all duration-200
                    {statusFilters.has(status)
                      ? status === 'completed' ? 'bg-green-600 text-white ring-2 ring-green-600 ring-offset-2 dark:ring-offset-gray-800'
                      : status === 'running' ? 'bg-blue-600 text-white ring-2 ring-blue-600 ring-offset-2 dark:ring-offset-gray-800'
                      : status === 'failed' ? 'bg-red-600 text-white ring-2 ring-red-600 ring-offset-2 dark:ring-offset-gray-800'
                      : status === 'cancelled' ? 'bg-yellow-600 text-white ring-2 ring-yellow-600 ring-offset-2 dark:ring-offset-gray-800'
                      : 'bg-gray-600 text-white ring-2 ring-gray-600 ring-offset-2 dark:ring-offset-gray-800'
                      : status === 'completed' ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50'
                      : status === 'running' ? 'bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50'
                      : status === 'failed' ? 'bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50'
                      : status === 'cancelled' ? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:hover:bg-yellow-900/50'
                      : 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600'
                    }"
                >
                  {status}
                </button>
              {/each}
            </div>
          </div>

          {#if activeFilterCount > 0}
            <button
              onclick={clearAllFilters}
              class="btn btn-secondary w-full text-sm"
            >
              Clear all filters
            </button>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Results summary -->
    {#if !loading && runs.length > 0}
      <div class="px-4 py-2 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 rounded-b-xl">
        <p class="text-sm text-gray-500 dark:text-gray-400">
          Showing <span class="font-medium text-gray-700 dark:text-gray-200">{filteredRuns.length}</span>
          {#if filteredRuns.length !== runs.length}
            of <span class="font-medium text-gray-700 dark:text-gray-200">{runs.length}</span>
          {/if}
          {filteredRuns.length === 1 ? 'run' : 'runs'}
        </p>
      </div>
    {/if}
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else if runs.length === 0}
    <div class="card p-12 text-center">
      <svg
        class="mx-auto h-16 w-16 text-gray-400 mb-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">No backup history</h3>
      <p class="text-gray-500 dark:text-gray-400">Run a backup job to see history here.</p>
    </div>
  {:else if filteredRuns.length === 0}
    <div class="card p-12 text-center">
      <svg
        class="mx-auto h-16 w-16 text-gray-400 mb-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z"
        />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">No matching runs</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-4">Try adjusting your filters to find what you're looking for.</p>
      <button onclick={clearAllFilters} class="btn btn-secondary">
        Clear all filters
      </button>
    </div>
  {:else}
    <div class="card overflow-hidden">
      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead class="bg-gray-50 dark:bg-gray-800/50">
            <tr>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Job</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Status</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase whitespace-nowrap">Started</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden sm:table-cell">
                Duration
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden md:table-cell">Files</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden md:table-cell">Size</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Actions</th>
            </tr>
          </thead>
          <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
            {#each filteredRuns as run (run.id)}
              <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                <td class="px-4 py-3 text-sm text-gray-900 dark:text-gray-100">{getJobName(run.job_id)}</td>
                <td class="px-4 py-3">
                  <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
                </td>
                <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">{formatDate(run.started_at)}</td>
                <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 hidden sm:table-cell">
                  {formatDuration(run.started_at, run.completed_at)}
                </td>
                <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">{run.files_transferred ?? '-'}</td>
                <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">{formatBytes(run.bytes_transferred)}</td>
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <button
                      onclick={() => selectRun(run)}
                      class="text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 text-sm font-medium"
                    >
                      Logs
                    </button>
                    <button
                      onclick={() => confirmDeleteRun(run.id)}
                      class="text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 text-sm p-1"
                      title="Delete this run"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>

<!-- Log Details Modal -->
{#if selectedRun}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-2 sm:p-4">
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-7xl h-[95vh] sm:h-[90vh] flex flex-col">
      <div class="p-3 sm:p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between gap-2">
        <div class="min-w-0 flex-1">
          <h3 class="text-base sm:text-lg font-semibold text-gray-900 dark:text-white truncate">
            {getJobName(selectedRun.job_id)} - Run #{selectedRun.id}
          </h3>
          <p class="text-xs sm:text-sm text-gray-500 dark:text-gray-400">{formatDate(selectedRun.started_at)}</p>
        </div>
        <button onclick={closeDetails} class="flex-shrink-0 p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700" aria-label="Close details">
          <svg class="w-5 h-5 sm:w-6 sm:h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
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
            onPageChange={handlePageChange}
          />
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Purge Confirmation Modal -->
{#if showPurgeConfirm}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-md w-full p-6">
      <div class="flex items-center gap-3 mb-4">
        <div class="w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center flex-shrink-0">
          <svg class="w-5 h-5 text-red-600 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Confirm Delete</h3>
      </div>
      <p class="text-gray-600 dark:text-gray-300 mb-6">{getPurgeMessage()}</p>
      <div class="flex justify-end gap-3">
        <button
          onclick={cancelPurge}
          class="btn btn-secondary"
          disabled={purging}
        >
          Cancel
        </button>
        <button
          onclick={executePurge}
          class="btn bg-red-600 text-white hover:bg-red-700"
          disabled={purging}
        >
          {#if purging}
            <div class="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
          {/if}
          Delete
        </button>
      </div>
    </div>
  </div>
{/if}

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

  const LOG_PAGE_SIZE = 500;
  const POLL_INTERVAL_MS = 3000;

  const logsTotalPages = $derived(Math.max(1, Math.ceil(logsTotal / LOG_PAGE_SIZE)));
  const hasRunningJobs = $derived(runs.some((r) => r.status === 'running'));

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
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Backup History</h1>

    <div class="flex items-center gap-3">
      {#if runs.length > 0}
        {#if selectedJobId}
          <button
            onclick={confirmPurgeJob}
            class="btn btn-secondary text-sm"
          >
            Clear Job Logs
          </button>
        {/if}
        <button
          onclick={confirmPurgeAll}
          class="btn btn-secondary text-sm"
        >
          Clear All
        </button>
      {/if}
      <select
        bind:value={selectedJobId}
        onchange={() => loadRuns()}
        class="input w-48"
      >
        <option value={null}>All Jobs</option>
        {#each $jobsStore.jobs as job}
          <option value={job.id}>{job.name}</option>
        {/each}
      </select>
    </div>
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
  {:else}
    <div class="card overflow-hidden">
      <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
        <thead class="bg-gray-50 dark:bg-gray-800/50">
          <tr>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Job</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Status</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Started</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
              Duration
            </th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Files</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Size</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Actions</th>
          </tr>
        </thead>
        <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
          {#each runs as run (run.id)}
            <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
              <td class="px-4 py-3 text-sm text-gray-900 dark:text-gray-100">{getJobName(run.job_id)}</td>
              <td class="px-4 py-3">
                <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{formatDate(run.started_at)}</td>
              <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
                {formatDuration(run.started_at, run.completed_at)}
              </td>
              <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{run.files_transferred ?? '-'}</td>
              <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{formatBytes(run.bytes_transferred)}</td>
              <td class="px-4 py-3">
                <div class="flex items-center gap-2">
                  <button
                    onclick={() => selectRun(run)}
                    class="text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300 text-sm font-medium"
                  >
                    View Logs
                  </button>
                  <button
                    onclick={() => confirmDeleteRun(run.id)}
                    class="text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 text-sm"
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
  {/if}
</div>

<!-- Log Details Modal -->
{#if selectedRun}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-4xl w-full h-[90vh] flex flex-col">
      <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            {getJobName(selectedRun.job_id)} - Run #{selectedRun.id}
          </h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">{formatDate(selectedRun.started_at)}</p>
        </div>
        <button onclick={closeDetails} class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300" aria-label="Close details">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      <div class="flex-1 min-h-[300px] overflow-hidden relative">
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

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { statusStore } from '../lib/stores/logs';
  import { api } from '../lib/api';
  import type { JobRun } from '../lib/types';
  import JobCard from '../components/jobs/JobCard.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { formatStatus } from '$lib/i18n/status';

  let recentRuns = $state<JobRun[]>([]);
  let loadingRuns = $state(true);
  let unsubscribeStatus: (() => void) | null = null;
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  const POLL_INTERVAL_MS = 3000;

  const hasRunningJobs = $derived($jobsStore.jobs.some((j) => j.last_run_status === 'running'));

  function startPolling() {
    if (pollInterval) return;
    pollInterval = setInterval(() => {
      jobsStore.refresh();
      loadRecentRuns(true);
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

  async function loadRecentRuns(silent = false) {
    if (!silent) loadingRuns = true;
    try {
      const jobs = await api.jobs.list();
      const allRuns: JobRun[] = [];
      for (const job of jobs.slice(0, 5)) {
        const runs = await api.runs.list(job.id, 3);
        allRuns.push(...runs);
      }
      recentRuns = allRuns.sort(
        (a, b) => new Date(b.started_at || 0).getTime() - new Date(a.started_at || 0).getTime()
      ).slice(0, 10);
    } catch {
      // Ignore
    } finally {
      if (!silent) loadingRuns = false;
    }
  }

  onMount(() => {
    jobsStore.load();
    statusStore.connect();

    // Subscribe to status updates and refresh jobs when status changes
    unsubscribeStatus = statusStore.subscribe(() => {
      jobsStore.refresh();
    });

    // Load recent runs from all jobs
    loadRecentRuns();

    return () => {
      statusStore.disconnect();
    };
  });

  onDestroy(() => {
    stopPolling();
    if (unsubscribeStatus) {
      unsubscribeStatus();
    }
  });

  function formatDate(date: string | null): string {
    if (!date) return m.common_never();
    return new Date(date).toLocaleString();
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
</script>

<div class="space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{m.dashboard_title()}</h1>
    <a href="#/jobs/new" class="btn btn-primary w-full sm:w-auto text-center">{m.dashboard_new_job()}</a>
  </div>

  <!-- Stats -->
  <div class="grid grid-cols-2 sm:grid-cols-2 md:grid-cols-4 gap-3 sm:gap-4">
    <div class="card p-4">
      <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_total_jobs()}</div>
      <div class="text-2xl font-bold text-gray-900 dark:text-white">{$jobsStore.jobs.length}</div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_active_jobs()}</div>
      <div class="text-2xl font-bold text-gray-900 dark:text-white">
        {$jobsStore.jobs.filter((j) => j.enabled).length}
      </div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_recent_runs()}</div>
      <div class="text-2xl font-bold text-gray-900 dark:text-white">{recentRuns.length}</div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_failed_runs()}</div>
      <div class="text-2xl font-bold text-red-600 dark:text-red-400">
        {recentRuns.filter((r) => r.status === 'failed').length}
      </div>
    </div>
  </div>

  <!-- Jobs Overview -->
  <div>
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{m.dashboard_backup_jobs()}</h2>
    {#if $jobsStore.loading}
      <div class="flex justify-center py-8">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      </div>
    {:else if $jobsStore.jobs.length === 0}
      <div class="card p-8 text-center">
        <p class="text-gray-500 dark:text-gray-400 mb-4">{m.dashboard_no_jobs()}</p>
        <a href="#/jobs/new" class="btn btn-primary">{m.dashboard_create_first()}</a>
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each $jobsStore.jobs as job (job.id)}
          <JobCard {job} />
        {/each}
      </div>
    {/if}
  </div>

  <!-- Recent Activity -->
  <div>
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{m.dashboard_recent_activity()}</h2>
    {#if loadingRuns}
      <div class="flex justify-center py-8">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      </div>
    {:else if recentRuns.length === 0}
      <div class="card p-8 text-center">
        <p class="text-gray-500 dark:text-gray-400">{m.dashboard_no_recent_runs()}</p>
      </div>
    {:else}
      <div class="card overflow-hidden">
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead class="bg-gray-50 dark:bg-gray-800/50">
              <tr>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">{m.history_table_job()}</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                  {m.history_table_status()}
                </th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase whitespace-nowrap">
                  {m.history_table_started()}
                </th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">{m.history_table_files()}</th>
              </tr>
            </thead>
            <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {#each recentRuns as run (run.id)}
                <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
                  <td class="px-4 py-3 text-sm text-gray-900 dark:text-gray-100">
                    {$jobsStore.jobs.find((j) => j.id === run.job_id)?.name || `Job #${run.job_id}`}
                  </td>
                  <td class="px-4 py-3">
                    <span class="badge {getStatusBadge(run.status)}">{formatStatus(run.status)}</span>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">{formatDate(run.started_at)}</td>
                  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{run.files_transferred ?? '-'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>
</div>

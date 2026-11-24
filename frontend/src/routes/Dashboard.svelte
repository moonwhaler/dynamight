<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { statusStore } from '../lib/stores/logs';
  import { api } from '../lib/api';
  import type { JobRun } from '../lib/types';
  import JobCard from '../components/jobs/JobCard.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { formatStatus } from '$lib/i18n/status';
  import { getStatusBadgeClass } from '$lib/i18n/status';
  import { formatDateTime, formatRelativeTime, formatTimeUntil } from '$lib/i18n/relativeTime';
  import { jobHealth, nextScheduledRun, oldestBackup } from '$lib/utils/backupHealth';
  import RunLogModal from '../components/logs/RunLogModal.svelte';

  let recentRuns = $state<JobRun[]>([]);
  let loadingRuns = $state(true);
  let unsubscribeStatus: (() => void) | null = null;
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  const POLL_INTERVAL_MS = 3000;

  /** Run whose log is open (clicked in Recent Activity) */
  let openRun = $state<JobRun | null>(null);

  // Re-evaluated every minute so "overdue" and relative times stay current
  let now = $state(Date.now());

  const activeJobs = $derived($jobsStore.jobs.filter((j) => j.enabled).length);
  const attention = $derived(
    $jobsStore.jobs
      .map((job) => ({ job, health: jobHealth(job, now) }))
      .filter(({ health }) => health.state !== 'ok')
  );
  const nextRun = $derived(nextScheduledRun($jobsStore.jobs));
  const oldest = $derived(oldestBackup($jobsStore.jobs));
  const oldestIsOverdue = $derived(
    oldest !== null && attention.some((a) => a.job.id === oldest.job.id && a.health.state === 'overdue')
  );

  const hasRunningJobs = $derived(
    $jobsStore.jobs.some((j) => j.last_run_status === 'running' || j.last_run_status === 'pending')
  );

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
      recentRuns = await api.runs.recent({ limit: 10 });
    } catch {
      // Ignore
    } finally {
      if (!silent) loadingRuns = false;
    }
  }

  onMount(() => {
    const clock = setInterval(() => (now = Date.now()), 60_000);
    jobsStore.load();
    statusStore.connect();

    unsubscribeStatus = statusStore.subscribe(() => {
      jobsStore.refresh();
      loadRecentRuns(true);
    });

    loadRecentRuns();

    return () => {
      clearInterval(clock);
      statusStore.disconnect();
    };
  });

  onDestroy(() => {
    stopPolling();
    if (unsubscribeStatus) {
      unsubscribeStatus();
    }
  });
</script>

<div class="space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{m.dashboard_title()}</h1>
    <a href="#/jobs/new" class="btn btn-primary w-full sm:w-auto text-center">{m.dashboard_new_job()}</a>
  </div>

  {#if $jobsStore.error && $jobsStore.jobs.length === 0}
    <div class="card p-8 text-center">
      <p class="text-red-600 dark:text-red-400 mb-4">{m.error_failed_load()}: {$jobsStore.error}</p>
      <button type="button" class="btn btn-secondary" onclick={() => jobsStore.load()}>{m.common_retry()}</button>
    </div>
  {:else if !$jobsStore.loading && $jobsStore.jobs.length > 0}
    <!-- Stats -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-3 sm:gap-4">
      <div class="card p-4">
        <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_active_jobs()}</div>
        <div class="text-2xl font-bold text-gray-900 dark:text-white">
          {activeJobs}<span class="ml-1 text-base font-normal text-gray-500 dark:text-gray-400">/ {$jobsStore.jobs.length}</span>
        </div>
      </div>
      <div class="card p-4">
        <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_attention()}</div>
        {#if attention.length > 0}
          <div class="text-2xl font-bold text-red-600 dark:text-red-400">{attention.length}</div>
        {:else}
          <div class="text-2xl font-bold text-green-600 dark:text-green-400">{m.dashboard_all_ok()}</div>
        {/if}
      </div>
      <div class="card p-4 min-w-0">
        <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_oldest_backup()}</div>
        {#if oldest}
          <div class="text-2xl font-bold truncate {oldestIsOverdue ? 'text-red-600 dark:text-red-400' : 'text-gray-900 dark:text-white'}">
            {formatRelativeTime(oldest.at)}
          </div>
          <div class="text-xs text-gray-500 dark:text-gray-400 truncate" title={oldest.job.name}>{oldest.job.name}</div>
        {:else}
          <div class="text-2xl font-bold text-gray-400 dark:text-gray-500">–</div>
          <div class="text-xs text-gray-500 dark:text-gray-400">{m.dashboard_no_schedules()}</div>
        {/if}
      </div>
      <div class="card p-4 min-w-0">
        <div class="text-sm text-gray-500 dark:text-gray-400">{m.dashboard_next_backup()}</div>
        {#if nextRun}
          <div class="text-2xl font-bold text-gray-900 dark:text-white truncate" title={formatDateTime(nextRun.at)}>
            {formatTimeUntil(nextRun.at)}
          </div>
          <div class="text-xs text-gray-500 dark:text-gray-400 truncate" title={nextRun.job.name}>{nextRun.job.name}</div>
        {:else}
          <div class="text-2xl font-bold text-gray-400 dark:text-gray-500">–</div>
          <div class="text-xs text-gray-500 dark:text-gray-400">{m.dashboard_no_schedules()}</div>
        {/if}
      </div>
    </div>

    {#if attention.length > 0}
      <div class="card p-4 border-red-300 dark:border-red-800">
        <h2 class="text-sm font-semibold text-red-700 dark:text-red-400 mb-2">{m.dashboard_attention()}</h2>
        <ul class="space-y-1">
          {#each attention as { job, health } (job.id)}
            <li class="text-sm">
              <a href="#/jobs/{job.id}" class="font-medium text-gray-900 dark:text-white hover:underline">{job.name}</a>
              <span class="text-gray-600 dark:text-gray-400">
                –
                {#if health.state === 'failed'}
                  {m.dashboard_attention_failed()}
                {:else if health.state === 'overdue' && health.lastSuccessAt}
                  {m.dashboard_attention_overdue({ time: formatRelativeTime(health.lastSuccessAt) })}
                {:else}
                  {m.dashboard_attention_never()}
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {/if}

  <!-- Jobs Overview -->
  <div>
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{m.dashboard_backup_jobs()}</h2>
    {#if $jobsStore.loading}
      <div class="flex justify-center py-8">
        <Spinner size="w-8 h-8" />
      </div>
    {:else if $jobsStore.jobs.length === 0 && !$jobsStore.error}
      <div class="card p-8 text-center">
        <p class="text-gray-500 dark:text-gray-400 mb-4">{m.dashboard_no_jobs()}</p>
        <a href="#/jobs/new" class="btn btn-primary">{m.dashboard_create_first()}</a>
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each $jobsStore.jobs as job (job.id)}
          <JobCard {job} onStatusChange={() => loadRecentRuns(true)} />
        {/each}
      </div>
    {/if}
  </div>

  <!-- Recent Activity -->
  <div>
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{m.dashboard_recent_activity()}</h2>
    {#if loadingRuns}
      <div class="flex justify-center py-8">
        <Spinner size="w-8 h-8" />
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
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">{m.history_table_status()}</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase whitespace-nowrap">{m.history_table_started()}</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">{m.history_table_files()}</th>
              </tr>
            </thead>
            <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {#each recentRuns as run (run.id)}
                <tr
                  class="hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer"
                  tabindex="0"
                  title={m.dashboard_open_log()}
                  onclick={() => (openRun = run)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      openRun = run;
                    }
                  }}
                >
                  <td class="px-4 py-3 text-sm text-gray-900 dark:text-gray-100">
                    {$jobsStore.jobs.find((j) => j.id === run.job_id)?.name || `Job #${run.job_id}`}
                  </td>
                  <td class="px-4 py-3">
                    <span class="badge {getStatusBadgeClass(run.status)}">{formatStatus(run.status)}</span>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">{formatDateTime(run.started_at)}</td>
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

{#if openRun}
  <RunLogModal runId={openRun.id} jobId={openRun.job_id} onClose={() => (openRun = null)} />
{/if}

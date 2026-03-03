<script lang="ts">
  import type { Job } from '../../lib/types';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import { get } from 'svelte/store';
  import RunLogModal from '../logs/RunLogModal.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import { formatRelativeTime } from '$lib/i18n/relativeTime';
  import { getStatusIndicator } from '$lib/i18n/status';
  import { formatBytes } from '$lib/utils/format';
  import { getDestinationLabel } from '$lib/utils/jobUtils';
  import * as m from '$lib/paraglide/messages.js';

  let { job, onStatusChange }: { job: Job; onStatusChange?: () => void } = $props();
  let starting = $state(false);
  let stopping = $state(false);
  let toggling = $state(false);
  let activeRunId = $state<number | null>(null);
  let loadingLogs = $state(false);
  let checkingSpace = $state(false);

  const isLocalDestination = $derived(job.destination_type === 'local');
  const isRunning = $derived(job.last_run_status === 'running' || job.last_run_status === 'pending');
  const statusInfo = $derived(getStatusIndicator(job.last_run_status));
  const timeAgo = $derived(formatRelativeTime(job.last_run_at));

  const hasStorageInfo = $derived(job.dest_storage_free != null && job.dest_storage_total != null);
  const storagePercentage = $derived(
    job.dest_storage_free != null && job.dest_storage_total != null && job.dest_storage_total > 0
      ? Math.round(((job.dest_storage_total - job.dest_storage_free) / job.dest_storage_total) * 100)
      : 0
  );

  async function handleRun() {
    if (starting || isRunning) return;
    starting = true;
    try {
      const result = await api.jobs.run(job.id);
      onStatusChange?.();
      if ($preferencesStore.showLogViewerAfterManualRun) {
        activeRunId = result.runId;
      }
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.job_error_start(), variant: 'error' });
    } finally {
      starting = false;
    }
  }

  async function handleStop() {
    if (stopping || !isRunning) return;

    if (get(preferencesStore).confirmKillProcess) {
      const confirmed = await confirm({
        title: m.kill_confirm_title(),
        message: m.kill_confirm_message(),
        confirmText: m.kill_confirm_button(),
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    stopping = true;
    try {
      await api.jobs.cancel(job.id);
      await jobsStore.refresh();
      onStatusChange?.();
    } catch {
      // Ignore errors
    } finally {
      stopping = false;
    }
  }

  async function handleViewLogs() {
    if (loadingLogs || !isRunning) return;
    loadingLogs = true;
    try {
      const runs = await api.runs.list(job.id, 1, 0);
      if (runs.length > 0 && (runs[0].status === 'running' || runs[0].status === 'pending')) {
        activeRunId = runs[0].id;
      }
    } catch {
      // Ignore errors
    } finally {
      loadingLogs = false;
    }
  }

  async function handleToggleEnabled(event: Event) {
    event.preventDefault();
    event.stopPropagation();
    if (toggling) return;
    toggling = true;
    try {
      const updatedJob = await api.jobs.update(job.id, { enabled: !job.enabled });
      jobsStore.updateJob(updatedJob);
    } catch {
      // Ignore
    } finally {
      toggling = false;
    }
  }

  async function handleCheckSpace() {
    if (checkingSpace || isRunning) return;
    checkingSpace = true;
    try {
      const result = await api.jobs.checkSpace(job.id);
      if (result.fits) {
        showToast({
          message: `${m.space_check_fits()}: ${m.space_check_transfer_size()} ${formatBytes(result.transfer_size)}, ${m.space_check_free_space()} ${formatBytes(result.destination_free)}`,
          variant: 'success',
        });
      } else {
        showToast({
          message: `${m.space_check_insufficient()}: ${m.space_check_deficit()} ${formatBytes(result.deficit ?? 0)}`,
          variant: 'error',
        });
      }
    } catch {
      showToast({ message: m.space_check_error(), variant: 'error' });
    } finally {
      checkingSpace = false;
    }
  }
</script>

<div class="card p-4 hover:shadow-md transition-shadow">
  <div class="flex items-start justify-between">
    <div class="flex-1 min-w-0">
      <a href="#/jobs/{job.id}" class="block">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white truncate hover:text-primary-600 dark:hover:text-primary-400 flex items-center gap-2">
          {job.name}
          {#if statusInfo.color}
            <span
              class="inline-block w-2.5 h-2.5 rounded-full {statusInfo.color} shrink-0"
              title="{statusInfo.label}{timeAgo ? ` (${timeAgo})` : ''}"
            ></span>
          {/if}
        </h3>
      </a>
      {#if job.description}
        <p class="text-sm text-gray-500 dark:text-gray-400 truncate mt-1">{job.description}</p>
      {/if}
    </div>
    <button
      onclick={handleToggleEnabled}
      disabled={toggling || isRunning}
      class="badge {isRunning ? 'badge-info' : job.enabled ? 'badge-success' : 'badge-gray'} ml-2 shrink-0 {isRunning ? '' : 'cursor-pointer hover:opacity-80'} transition-opacity"
      title={isRunning ? m.job_card_currently_running() : job.enabled ? m.job_card_click_disable() : m.job_card_click_enable()}
    >
      {toggling ? '...' : isRunning ? m.common_running() : job.enabled ? m.common_active() : m.common_disabled()}
    </button>
  </div>

  <div class="mt-4 space-y-2 text-sm text-gray-600 dark:text-gray-400">
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
      </svg>
      <span class="truncate">{m.jobs_sources_count({ count: job.source_dirs.length })}</span>
    </div>
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
      </svg>
      <span class="truncate">{getDestinationLabel(job)}</span>
    </div>
    {#if hasStorageInfo && job.dest_storage_total}
      <div
        class="flex items-center gap-2"
        title={m.storage_tooltip({ free: formatBytes(job.dest_storage_free ?? 0), total: formatBytes(job.dest_storage_total) })}
      >
        <svg class="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
        </svg>
        <div class="flex-1 min-w-0">
          <div class="flex items-center justify-between text-xs mb-0.5">
            <span class="truncate">{formatBytes(job.dest_storage_free ?? 0)} {m.storage_free_short()}</span>
          </div>
          <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5">
            <div
              class="h-1.5 rounded-full transition-all {storagePercentage >= 90 ? 'bg-red-500' : storagePercentage >= 75 ? 'bg-orange-500' : 'bg-green-500'}"
              style="width: {storagePercentage}%"
            ></div>
          </div>
        </div>
      </div>
    {/if}
  </div>

  <div class="mt-4 flex items-center justify-between">
    <div class="flex gap-1">
      {#if job.sync_deletes}
        <span class="badge badge-warning text-xs">{m.job_card_badge_mirror()}</span>
      {/if}
      {#if job.compress}
        <span class="badge badge-info text-xs">{m.job_card_badge_compress()}</span>
      {/if}
      {#if job.dry_run}
        <span class="badge badge-gray text-xs">{m.job_card_badge_dry_run()}</span>
      {/if}
    </div>
    {#if isRunning || stopping}
      <div class="flex items-center gap-1.5">
        <button
          onclick={handleViewLogs}
          disabled={loadingLogs}
          class="btn btn-sm w-9 h-9 p-0 flex items-center justify-center bg-blue-50 text-blue-600 hover:bg-blue-100 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50 border border-blue-200 dark:border-blue-800"
          title={m.job_card_tooltip_view_logs()}
        >
          {#if loadingLogs}
            <Spinner />
          {:else}
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 7.5l3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0021 18V6a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 003 6v12a2.25 2.25 0 002.25 2.25z" />
            </svg>
          {/if}
        </button>
        <button
          onclick={handleStop}
          disabled={stopping}
          class="btn btn-sm btn-danger w-9 h-9 p-0 flex items-center justify-center"
          title={m.job_card_tooltip_stop()}
        >
          {#if stopping}
            <Spinner />
          {:else}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <rect x="6" y="6" width="12" height="12" rx="1" />
            </svg>
          {/if}
        </button>
      </div>
    {:else}
      <div class="flex items-center gap-1.5">
        {#if isLocalDestination}
          <button
            onclick={handleCheckSpace}
            disabled={checkingSpace}
            class="btn btn-sm w-9 h-9 p-0 flex items-center justify-center bg-gray-50 text-gray-600 hover:bg-gray-100 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700 border border-gray-200 dark:border-gray-700"
            title={m.space_check_button()}
          >
            {#if checkingSpace}
              <Spinner />
            {:else}
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" />
              </svg>
            {/if}
          </button>
        {/if}
        <button
          onclick={handleRun}
          disabled={starting || !job.enabled}
          class="btn btn-sm btn-secondary w-9 h-9 p-0 flex items-center justify-center"
          title={!job.enabled ? m.job_card_tooltip_enable() : m.job_card_tooltip_start()}
        >
          {#if starting}
            <Spinner />
          {:else}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z" />
            </svg>
          {/if}
        </button>
      </div>
    {/if}
  </div>
</div>

{#if activeRunId !== null}
  <RunLogModal runId={activeRunId} jobId={job.id} onClose={() => { activeRunId = null; }} />
{/if}

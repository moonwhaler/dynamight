<script lang="ts">
  import type { Job } from '../../lib/types';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import RunLogModal from '../logs/RunLogModal.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { job }: { job: Job } = $props();
  let running = $state(false);
  let toggling = $state(false);
  let activeRunId = $state<number | null>(null);

  function getStatusIndicator(status: string | null | undefined): { color: string; label: string } {
    switch (status) {
      case 'completed':
        return { color: 'bg-green-500', label: m.job_card_last_run_succeeded() };
      case 'failed':
        return { color: 'bg-red-500', label: m.job_card_last_run_failed() };
      case 'running':
        return { color: 'bg-blue-500', label: m.job_card_currently_running() };
      case 'cancelled':
        return { color: 'bg-orange-500', label: m.job_card_last_run_cancelled() };
      case 'pending':
        return { color: 'bg-yellow-500', label: m.job_card_run_pending() };
      default:
        return { color: '', label: '' };
    }
  }

  function formatRelativeTime(dateStr: string | null | undefined): string {
    if (!dateStr) return '';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return m.time_just_now();
    if (diffMins < 60) return m.time_minutes_ago({ count: diffMins });
    if (diffHours < 24) return m.time_hours_ago({ count: diffHours });
    if (diffDays < 7) return m.time_days_ago({ count: diffDays });
    return date.toLocaleDateString();
  }

  const statusInfo = $derived(getStatusIndicator(job.last_run_status));
  const timeAgo = $derived(formatRelativeTime(job.last_run_at));

  async function handleRun() {
    if (running) return;
    running = true;
    try {
      const result = await api.jobs.run(job.id);
      if ($preferencesStore.showLogViewerAfterManualRun) {
        activeRunId = result.runId;
      }
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.job_error_start(), variant: 'error' });
    } finally {
      running = false;
    }
  }

  function closeRunModal() {
    activeRunId = null;
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
      disabled={toggling}
      class="badge {job.enabled ? 'badge-success' : 'badge-gray'} ml-2 shrink-0 cursor-pointer hover:opacity-80 transition-opacity"
      title={job.enabled ? m.job_card_click_disable() : m.job_card_click_enable()}
    >
      {toggling ? '...' : job.enabled ? m.common_active() : m.common_disabled()}
    </button>
  </div>

  <div class="mt-4 space-y-2 text-sm text-gray-600 dark:text-gray-400">
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
        />
      </svg>
      <span class="truncate">{m.jobs_sources_count({ count: job.source_dirs.length })}</span>
    </div>
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4"
        />
      </svg>
      <span class="truncate">{job.mount_point}/{job.backup_subdir}</span>
    </div>
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
    <button
      onclick={handleRun}
      disabled={running || !job.enabled}
      class="btn btn-sm btn-secondary"
      title={!job.enabled ? m.job_card_tooltip_enable() : m.job_card_tooltip_start()}
    >
      {running ? m.job_btn_starting() : m.common_run()}
    </button>
  </div>
</div>

<!-- Run Log Modal -->
{#if activeRunId !== null}
  <RunLogModal
    runId={activeRunId}
    jobId={job.id}
    onClose={closeRunModal}
  />
{/if}

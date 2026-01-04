<script lang="ts">
  import type { Job } from '../../lib/types';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import RunLogModal from '../logs/RunLogModal.svelte';

  let { job }: { job: Job } = $props();
  let running = $state(false);
  let toggling = $state(false);
  let activeRunId = $state<number | null>(null);

  function getStatusIndicator(status: string | null | undefined): { color: string; label: string } {
    switch (status) {
      case 'completed':
        return { color: 'bg-green-500', label: 'Last run succeeded' };
      case 'failed':
        return { color: 'bg-red-500', label: 'Last run failed' };
      case 'running':
        return { color: 'bg-blue-500', label: 'Currently running' };
      case 'cancelled':
        return { color: 'bg-orange-500', label: 'Last run was cancelled' };
      case 'pending':
        return { color: 'bg-yellow-500', label: 'Run pending' };
      default:
        return { color: '', label: '' };
    }
  }

  function formatRelativeTime(dateStr: string | null | undefined): string {
    if (!dateStr) return 'Never';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
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
    } catch {
      // Ignore
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

<tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
  <td class="px-4 py-3">
    <a href="#/jobs/{job.id}" class="flex items-center gap-2 group">
      {#if statusInfo.color}
        <span
          class="inline-block w-2.5 h-2.5 rounded-full {statusInfo.color} shrink-0"
          title="{statusInfo.label}{timeAgo ? ` (${timeAgo})` : ''}"
        ></span>
      {:else}
        <span class="inline-block w-2.5 h-2.5 rounded-full bg-gray-300 dark:bg-gray-600 shrink-0"></span>
      {/if}
      <span class="text-sm font-medium text-gray-900 dark:text-gray-100 group-hover:text-primary-600 dark:group-hover:text-primary-400">
        {job.name}
      </span>
    </a>
  </td>
  <td class="px-4 py-3">
    <button
      onclick={handleToggleEnabled}
      disabled={toggling}
      class="badge {job.enabled ? 'badge-success' : 'badge-gray'} cursor-pointer hover:opacity-80 transition-opacity"
      title={job.enabled ? 'Click to disable' : 'Click to enable'}
    >
      {toggling ? '...' : job.enabled ? 'Active' : 'Disabled'}
    </button>
  </td>
  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 hidden md:table-cell">
    {job.source_dirs.length} source(s)
  </td>
  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 truncate max-w-[200px] hidden lg:table-cell" title="{job.mount_point}/{job.backup_subdir}">
    {job.mount_point}/{job.backup_subdir}
  </td>
  <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap hidden sm:table-cell">
    {timeAgo}
  </td>
  <td class="px-4 py-3 hidden xl:table-cell">
    <div class="flex gap-1">
      {#if job.sync_deletes}
        <span class="badge badge-warning text-xs">Mirror</span>
      {/if}
      {#if job.compress}
        <span class="badge badge-info text-xs">Compress</span>
      {/if}
      {#if job.dry_run}
        <span class="badge badge-gray text-xs">Dry Run</span>
      {/if}
    </div>
  </td>
  <td class="px-4 py-3">
    <button
      onclick={handleRun}
      disabled={running || !job.enabled}
      class="btn btn-sm btn-secondary"
      title={!job.enabled ? 'Enable job to run' : 'Start backup job'}
    >
      {running ? 'Starting...' : 'Run'}
    </button>
  </td>
</tr>

<!-- Run Log Modal -->
{#if activeRunId !== null}
  <RunLogModal
    runId={activeRunId}
    jobId={job.id}
    onClose={closeRunModal}
  />
{/if}

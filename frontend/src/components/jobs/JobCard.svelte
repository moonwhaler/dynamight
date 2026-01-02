<script lang="ts">
  import type { Job } from '../../lib/types';
  import { api } from '../../lib/api';

  let { job }: { job: Job } = $props();
  let running = $state(false);

  async function handleRun() {
    if (running) return;
    running = true;
    try {
      await api.jobs.run(job.id);
    } catch {
      // Ignore
    } finally {
      running = false;
    }
  }
</script>

<div class="card p-4 hover:shadow-md transition-shadow">
  <div class="flex items-start justify-between">
    <div class="flex-1 min-w-0">
      <a href="#/jobs/{job.id}" class="block">
        <h3 class="text-lg font-semibold text-gray-900 truncate hover:text-primary-600">
          {job.name}
        </h3>
      </a>
      {#if job.description}
        <p class="text-sm text-gray-500 truncate mt-1">{job.description}</p>
      {/if}
    </div>
    <span
      class="badge {job.enabled ? 'badge-success' : 'badge-gray'} ml-2 shrink-0"
    >
      {job.enabled ? 'Active' : 'Disabled'}
    </span>
  </div>

  <div class="mt-4 space-y-2 text-sm text-gray-600">
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
        />
      </svg>
      <span class="truncate">{job.source_dirs.length} source(s)</span>
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
        <span class="badge badge-warning text-xs">Mirror</span>
      {/if}
      {#if job.compress}
        <span class="badge badge-info text-xs">Compress</span>
      {/if}
      {#if job.dry_run}
        <span class="badge badge-gray text-xs">Dry Run</span>
      {/if}
    </div>
    <button
      onclick={handleRun}
      disabled={running || !job.enabled}
      class="btn btn-secondary text-sm py-1 px-3"
    >
      {running ? 'Running...' : 'Run'}
    </button>
  </div>
</div>

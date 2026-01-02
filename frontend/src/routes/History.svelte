<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import type { Job, JobRun, LogEntry } from '../lib/types';
  import LogViewer from '../components/logs/LogViewer.svelte';

  let runs = $state<JobRun[]>([]);
  let loading = $state(true);
  let selectedJobId = $state<number | null>(null);
  let selectedRun = $state<JobRun | null>(null);
  let logs = $state<LogEntry[]>([]);
  let loadingLogs = $state(false);

  onMount(async () => {
    await jobsStore.load();
    await loadRuns();
  });

  async function loadRuns() {
    loading = true;
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
      loading = false;
    }
  }

  async function selectRun(run: JobRun) {
    selectedRun = run;
    loadingLogs = true;
    try {
      logs = await api.runs.logs(run.id);
    } catch {
      logs = [];
    } finally {
      loadingLogs = false;
    }
  }

  function closeDetails() {
    selectedRun = null;
    logs = [];
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
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-gray-900">Backup History</h1>

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
      <h3 class="text-lg font-medium text-gray-900 mb-2">No backup history</h3>
      <p class="text-gray-500">Run a backup job to see history here.</p>
    </div>
  {:else}
    <div class="card overflow-hidden">
      <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
          <tr>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Job</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Status</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Started</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
              Duration
            </th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Files</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Size</th>
            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Actions</th>
          </tr>
        </thead>
        <tbody class="bg-white divide-y divide-gray-200">
          {#each runs as run (run.id)}
            <tr class="hover:bg-gray-50">
              <td class="px-4 py-3 text-sm text-gray-900">{getJobName(run.job_id)}</td>
              <td class="px-4 py-3">
                <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
              </td>
              <td class="px-4 py-3 text-sm text-gray-500">{formatDate(run.started_at)}</td>
              <td class="px-4 py-3 text-sm text-gray-500">
                {formatDuration(run.started_at, run.completed_at)}
              </td>
              <td class="px-4 py-3 text-sm text-gray-500">{run.files_transferred ?? '-'}</td>
              <td class="px-4 py-3 text-sm text-gray-500">{formatBytes(run.bytes_transferred)}</td>
              <td class="px-4 py-3">
                <button
                  onclick={() => selectRun(run)}
                  class="text-primary-600 hover:text-primary-700 text-sm font-medium"
                >
                  View Logs
                </button>
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
    <div class="bg-white rounded-xl shadow-xl max-w-4xl w-full max-h-[90vh] flex flex-col">
      <div class="p-4 border-b flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900">
            {getJobName(selectedRun.job_id)} - Run #{selectedRun.id}
          </h3>
          <p class="text-sm text-gray-500">{formatDate(selectedRun.started_at)}</p>
        </div>
        <button onclick={closeDetails} class="text-gray-400 hover:text-gray-600">
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

      <div class="flex-1 overflow-hidden">
        {#if loadingLogs}
          <div class="flex justify-center py-12">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else}
          <LogViewer {logs} />
        {/if}
      </div>
    </div>
  </div>
{/if}

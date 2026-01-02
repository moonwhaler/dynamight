<script lang="ts">
  import { onMount } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { statusStore } from '../lib/stores/logs';
  import { api } from '../lib/api';
  import type { JobRun } from '../lib/types';
  import JobCard from '../components/jobs/JobCard.svelte';

  let recentRuns = $state<JobRun[]>([]);
  let loadingRuns = $state(true);

  onMount(() => {
    jobsStore.load();
    statusStore.connect();

    // Load recent runs from all jobs
    (async () => {
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
        loadingRuns = false;
      }
    })();

    return () => {
      statusStore.disconnect();
    };
  });

  function formatDate(date: string | null): string {
    if (!date) return 'Never';
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
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-gray-900">Dashboard</h1>
    <a href="#/jobs/new" class="btn btn-primary"> New Job </a>
  </div>

  <!-- Stats -->
  <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
    <div class="card p-4">
      <div class="text-sm text-gray-500">Total Jobs</div>
      <div class="text-2xl font-bold text-gray-900">{$jobsStore.jobs.length}</div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500">Active Jobs</div>
      <div class="text-2xl font-bold text-gray-900">
        {$jobsStore.jobs.filter((j) => j.enabled).length}
      </div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500">Recent Runs</div>
      <div class="text-2xl font-bold text-gray-900">{recentRuns.length}</div>
    </div>
    <div class="card p-4">
      <div class="text-sm text-gray-500">Failed Runs</div>
      <div class="text-2xl font-bold text-red-600">
        {recentRuns.filter((r) => r.status === 'failed').length}
      </div>
    </div>
  </div>

  <!-- Jobs Overview -->
  <div>
    <h2 class="text-lg font-semibold text-gray-900 mb-4">Backup Jobs</h2>
    {#if $jobsStore.loading}
      <div class="flex justify-center py-8">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      </div>
    {:else if $jobsStore.jobs.length === 0}
      <div class="card p-8 text-center">
        <p class="text-gray-500 mb-4">No backup jobs configured yet.</p>
        <a href="#/jobs/new" class="btn btn-primary">Create your first job</a>
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
    <h2 class="text-lg font-semibold text-gray-900 mb-4">Recent Activity</h2>
    {#if loadingRuns}
      <div class="flex justify-center py-8">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      </div>
    {:else if recentRuns.length === 0}
      <div class="card p-8 text-center">
        <p class="text-gray-500">No recent backup runs.</p>
      </div>
    {:else}
      <div class="card overflow-hidden">
        <table class="min-w-full divide-y divide-gray-200">
          <thead class="bg-gray-50">
            <tr>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Job</th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Status
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Started
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Files</th>
            </tr>
          </thead>
          <tbody class="bg-white divide-y divide-gray-200">
            {#each recentRuns as run (run.id)}
              <tr class="hover:bg-gray-50">
                <td class="px-4 py-3 text-sm text-gray-900">
                  {$jobsStore.jobs.find((j) => j.id === run.job_id)?.name || `Job #${run.job_id}`}
                </td>
                <td class="px-4 py-3">
                  <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
                </td>
                <td class="px-4 py-3 text-sm text-gray-500">{formatDate(run.started_at)}</td>
                <td class="px-4 py-3 text-sm text-gray-500">{run.files_transferred ?? '-'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

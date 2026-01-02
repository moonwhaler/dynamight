<script lang="ts">
  import { onMount } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import JobCard from '../components/jobs/JobCard.svelte';

  onMount(() => {
    jobsStore.load();
  });
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-gray-900">Backup Jobs</h1>
    <a href="#/jobs/new" class="btn btn-primary"> New Job </a>
  </div>

  {#if $jobsStore.loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else if $jobsStore.error}
    <div class="card p-8 text-center">
      <p class="text-red-600 mb-4">{$jobsStore.error}</p>
      <button onclick={() => jobsStore.load()} class="btn btn-secondary">Retry</button>
    </div>
  {:else if $jobsStore.jobs.length === 0}
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
          d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
        />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 mb-2">No backup jobs</h3>
      <p class="text-gray-500 mb-6">Get started by creating your first backup job.</p>
      <a href="#/jobs/new" class="btn btn-primary">Create Job</a>
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each $jobsStore.jobs as job (job.id)}
        <JobCard {job} />
      {/each}
    </div>
  {/if}
</div>

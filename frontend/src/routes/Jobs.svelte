<script lang="ts">
  import { onMount } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { viewPreferencesStore } from '../lib/stores/viewPreferences';
  import JobCard from '../components/jobs/JobCard.svelte';
  import JobListRow from '../components/jobs/JobListRow.svelte';

  // Filter state
  let searchQuery = $state('');
  let statusFilters = $state<Set<string>>(new Set());
  let enabledFilter = $state<'all' | 'enabled' | 'disabled'>('all');
  let showFilters = $state(false);

  const allStatuses = ['completed', 'running', 'failed', 'cancelled', 'pending'] as const;

  // Filtered jobs based on all filter criteria
  const filteredJobs = $derived.by(() => {
    let result = $jobsStore.jobs;

    // Filter by search query (job name or description)
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      result = result.filter((job) => {
        return job.name.toLowerCase().includes(query) ||
               (job.description && job.description.toLowerCase().includes(query));
      });
    }

    // Filter by last run status
    if (statusFilters.size > 0) {
      result = result.filter((job) => {
        if (!job.last_run_status) return false;
        return statusFilters.has(job.last_run_status);
      });
    }

    // Filter by enabled state
    if (enabledFilter === 'enabled') {
      result = result.filter((job) => job.enabled);
    } else if (enabledFilter === 'disabled') {
      result = result.filter((job) => !job.enabled);
    }

    return result;
  });

  const activeFilterCount = $derived(
    (searchQuery.trim() ? 1 : 0) +
    (statusFilters.size > 0 ? 1 : 0) +
    (enabledFilter !== 'all' ? 1 : 0)
  );

  function toggleStatus(status: string) {
    const newFilters = new Set(statusFilters);
    if (newFilters.has(status)) {
      newFilters.delete(status);
    } else {
      newFilters.add(status);
    }
    statusFilters = newFilters;
  }

  function clearAllFilters() {
    searchQuery = '';
    statusFilters = new Set();
    enabledFilter = 'all';
  }

  onMount(() => {
    jobsStore.load();
  });
</script>

<div class="space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Backup Jobs</h1>
    <a href="#/jobs/new" class="btn btn-primary w-full sm:w-auto text-center"> New Job </a>
  </div>

  {#if $jobsStore.loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else if $jobsStore.error}
    <div class="card p-8 text-center">
      <p class="text-red-600 dark:text-red-400 mb-4">{$jobsStore.error}</p>
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
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">No backup jobs</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-6">Get started by creating your first backup job.</p>
      <a href="#/jobs/new" class="btn btn-primary">Create Job</a>
    </div>
  {:else}
    <!-- Filter Bar -->
    <div class="card">
      <div class="p-4">
        <!-- Main filter row -->
        <div class="flex flex-col lg:flex-row gap-4">
          <!-- Search input -->
          <div class="relative flex-1 min-w-0">
            <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              type="text"
              bind:value={searchQuery}
              placeholder="Search jobs..."
              class="input pl-10"
            />
          </div>

          <!-- Enabled filter -->
          <select
            bind:value={enabledFilter}
            class="input lg:w-40"
          >
            <option value="all">All Jobs</option>
            <option value="enabled">Active Only</option>
            <option value="disabled">Disabled Only</option>
          </select>

          <!-- Filter toggle button (mobile/tablet) -->
          <button
            onclick={() => showFilters = !showFilters}
            class="btn btn-secondary flex items-center justify-center gap-2 lg:hidden"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
            </svg>
            Filters
            {#if activeFilterCount > 0}
              <span class="bg-primary-600 text-white text-xs font-medium px-2 py-0.5 rounded-full">{activeFilterCount}</span>
            {/if}
          </button>

          <!-- View toggle buttons -->
          <div class="hidden lg:flex items-center border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
            <button
              onclick={() => viewPreferencesStore.setViewMode('grid')}
              class="p-2 transition-colors {$viewPreferencesStore === 'grid'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
              title="Grid view"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
              </svg>
            </button>
            <button
              onclick={() => viewPreferencesStore.setViewMode('list')}
              class="p-2 transition-colors {$viewPreferencesStore === 'list'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
              title="List view"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Desktop: Status filter chips -->
        <div class="hidden lg:flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
          <span class="text-sm text-gray-500 dark:text-gray-400">Last Run:</span>
          <div class="flex flex-wrap gap-2">
            {#each allStatuses as status}
              <button
                onclick={() => toggleStatus(status)}
                class="px-3 py-1.5 rounded-full text-sm font-medium transition-all duration-200
                  {statusFilters.has(status)
                    ? status === 'completed' ? 'bg-green-600 text-white ring-2 ring-green-600 ring-offset-2 dark:ring-offset-gray-800'
                    : status === 'running' ? 'bg-blue-600 text-white ring-2 ring-blue-600 ring-offset-2 dark:ring-offset-gray-800'
                    : status === 'failed' ? 'bg-red-600 text-white ring-2 ring-red-600 ring-offset-2 dark:ring-offset-gray-800'
                    : status === 'cancelled' ? 'bg-yellow-600 text-white ring-2 ring-yellow-600 ring-offset-2 dark:ring-offset-gray-800'
                    : 'bg-gray-600 text-white ring-2 ring-gray-600 ring-offset-2 dark:ring-offset-gray-800'
                    : status === 'completed' ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50'
                    : status === 'running' ? 'bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50'
                    : status === 'failed' ? 'bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50'
                    : status === 'cancelled' ? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:hover:bg-yellow-900/50'
                    : 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600'
                  }"
              >
                {status}
              </button>
            {/each}
          </div>

          {#if activeFilterCount > 0}
            <button
              onclick={clearAllFilters}
              class="ml-auto text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 flex items-center gap-1"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
              Clear filters
            </button>
          {/if}
        </div>

        <!-- Mobile/Tablet: Expanded filters -->
        {#if showFilters}
          <div class="lg:hidden mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 space-y-4">
            <!-- View toggle -->
            <div class="space-y-2">
              <span class="label">View</span>
              <div class="flex items-center border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden w-fit">
                <button
                  onclick={() => viewPreferencesStore.setViewMode('grid')}
                  class="p-2 transition-colors {$viewPreferencesStore === 'grid'
                    ? 'bg-primary-600 text-white'
                    : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
                  title="Grid view"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
                  </svg>
                </button>
                <button
                  onclick={() => viewPreferencesStore.setViewMode('list')}
                  class="p-2 transition-colors {$viewPreferencesStore === 'list'
                    ? 'bg-primary-600 text-white'
                    : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
                  title="List view"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- Status filters -->
            <div class="space-y-2">
              <span class="label">Last Run Status</span>
              <div class="flex flex-wrap gap-2">
                {#each allStatuses as status}
                  <button
                    onclick={() => toggleStatus(status)}
                    class="px-3 py-1.5 rounded-full text-sm font-medium transition-all duration-200
                      {statusFilters.has(status)
                        ? status === 'completed' ? 'bg-green-600 text-white ring-2 ring-green-600 ring-offset-2 dark:ring-offset-gray-800'
                        : status === 'running' ? 'bg-blue-600 text-white ring-2 ring-blue-600 ring-offset-2 dark:ring-offset-gray-800'
                        : status === 'failed' ? 'bg-red-600 text-white ring-2 ring-red-600 ring-offset-2 dark:ring-offset-gray-800'
                        : status === 'cancelled' ? 'bg-yellow-600 text-white ring-2 ring-yellow-600 ring-offset-2 dark:ring-offset-gray-800'
                        : 'bg-gray-600 text-white ring-2 ring-gray-600 ring-offset-2 dark:ring-offset-gray-800'
                        : status === 'completed' ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50'
                        : status === 'running' ? 'bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50'
                        : status === 'failed' ? 'bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50'
                        : status === 'cancelled' ? 'bg-yellow-100 text-yellow-700 hover:bg-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:hover:bg-yellow-900/50'
                        : 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600'
                      }"
                  >
                    {status}
                  </button>
                {/each}
              </div>
            </div>

            {#if activeFilterCount > 0}
              <button
                onclick={clearAllFilters}
                class="btn btn-secondary w-full text-sm"
              >
                Clear all filters
              </button>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Results summary -->
      {#if filteredJobs.length !== $jobsStore.jobs.length}
        <div class="px-4 py-2 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 rounded-b-xl">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            Showing <span class="font-medium text-gray-700 dark:text-gray-200">{filteredJobs.length}</span>
            of <span class="font-medium text-gray-700 dark:text-gray-200">{$jobsStore.jobs.length}</span>
            {filteredJobs.length === 1 ? 'job' : 'jobs'}
          </p>
        </div>
      {/if}
    </div>

    <!-- Jobs display -->
    {#if filteredJobs.length === 0}
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
            d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z"
          />
        </svg>
        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">No matching jobs</h3>
        <p class="text-gray-500 dark:text-gray-400 mb-4">Try adjusting your filters to find what you're looking for.</p>
        <button onclick={clearAllFilters} class="btn btn-secondary">
          Clear all filters
        </button>
      </div>
    {:else if $viewPreferencesStore === 'grid'}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each filteredJobs as job (job.id)}
          <JobCard {job} />
        {/each}
      </div>
    {:else}
      <div class="card overflow-hidden">
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead class="bg-gray-50 dark:bg-gray-800/50">
              <tr>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Job</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Status</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden md:table-cell">Sources</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden lg:table-cell">Destination</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase whitespace-nowrap hidden sm:table-cell">Last Run</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase hidden xl:table-cell">Options</th>
                <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Actions</th>
              </tr>
            </thead>
            <tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {#each filteredJobs as job (job.id)}
                <JobListRow {job} />
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  {/if}
</div>

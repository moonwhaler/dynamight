<script lang="ts">
  import { onMount } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { viewPreferencesStore } from '../lib/stores/viewPreferences';
  import { createTableSortStore } from '../lib/stores/tableSortStore';
  import { jobStatus, matchesFilter, runsByJob, STATE_ORDER, type JobStatus, type StateFilter } from '../lib/utils/jobStatus';
  import type { Job } from '../lib/types';
  import PageHeader from '../components/ui/PageHeader.svelte';
  import EmptyState from '../components/ui/EmptyState.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import JobTable from '../components/jobs/JobTable.svelte';
  import JobTile from '../components/jobs/JobTile.svelte';
  import * as m from '$lib/paraglide/messages.js';

  type SortKey = 'name' | 'status' | 'last_success' | 'next_run';
  const sortStore = createTableSortStore<SortKey>({ storageKey: 'dynamight-jobs-sort-v2', defaultSortBy: 'status' });

  let query = $state('');
  let filter = $state<StateFilter | null>(null);
  let now = $state(Date.now());

  onMount(() => {
    const stop = jobsStore.live();
    const clock = setInterval(() => (now = Date.now()), 60_000);
    return () => {
      stop();
      clearInterval(clock);
    };
  });

  const jobs = $derived($jobsStore.jobs);
  const statuses = $derived(new Map<number, JobStatus>(jobs.map((j) => [j.id, jobStatus(j, now)])));
  const runs = $derived(runsByJob($jobsStore.runs));

  const FILTERS: { id: StateFilter; label: () => string }[] = [
    { id: 'attention', label: m.dashboard_attention },
    { id: 'running', label: m.state_running },
    { id: 'ok', label: m.jobs_filter_ok },
    { id: 'disabled', label: m.state_disabled },
  ];
  const counts = $derived(
    Object.fromEntries(FILTERS.map((f) => [f.id, jobs.filter((j) => matchesFilter(statuses.get(j.id)!, f.id)).length])),
  );

  function nextRunMs(job: Job): number {
    const times = (job.schedules ?? []).filter((s) => job.enabled && s.enabled && s.next_run_at).map((s) => Date.parse(s.next_run_at!));
    return times.length ? Math.min(...times) : Infinity;
  }

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = jobs.filter(
      (j) =>
        (!q || j.name.toLowerCase().includes(q) || (j.description ?? '').toLowerCase().includes(q)) &&
        (!filter || matchesFilter(statuses.get(j.id)!, filter)),
    );
    const { sortBy, sortOrder } = $sortStore;
    const dir = sortOrder === 'asc' ? 1 : -1;
    return list.sort((a, b) => {
      let c = 0;
      switch (sortBy) {
        case 'status':
          c = STATE_ORDER[statuses.get(a.id)!.state] - STATE_ORDER[statuses.get(b.id)!.state];
          break;
        case 'last_success':
          c = Date.parse(a.last_success_at ?? '1970-01-01') - Date.parse(b.last_success_at ?? '1970-01-01');
          break;
        case 'next_run':
          c = nextRunMs(a) - nextRunMs(b);
          break;
      }
      return c * dir || a.name.localeCompare(b.name);
    });
  });

  const attentionCount = $derived(counts.attention ?? 0);
</script>

<PageHeader title={m.nav_jobs()}>
  {#snippet subtitle()}
    {m.jobs_count({ count: jobs.length })}{#if attentionCount > 0}
      <span aria-hidden="true">&nbsp;·&nbsp;</span><button type="button" class="text-red-600 dark:text-red-400 hover:underline" onclick={() => (filter = 'attention')}>
        {m.jobs_attention_count({ count: attentionCount })}
      </button>{/if}
  {/snippet}
  {#snippet actions()}
    <div class="inline-flex rounded-lg border border-gray-300 dark:border-gray-600 overflow-hidden" role="group" aria-label={m.jobs_view_label()}>
      <button
        type="button"
        class="btn btn-icon rounded-none border-0 {$viewPreferencesStore === 'list' ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white' : 'btn-ghost'}"
        aria-pressed={$viewPreferencesStore === 'list'}
        aria-label={m.jobs_view_list()}
        title={m.jobs_view_list()}
        onclick={() => viewPreferencesStore.setViewMode('list')}
      >
        <Icon name="list" />
      </button>
      <button
        type="button"
        class="btn btn-icon rounded-none border-0 border-l border-gray-300 dark:border-gray-600 {$viewPreferencesStore === 'grid' ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white' : 'btn-ghost'}"
        aria-pressed={$viewPreferencesStore === 'grid'}
        aria-label={m.jobs_view_grid()}
        title={m.jobs_view_grid()}
        onclick={() => viewPreferencesStore.setViewMode('grid')}
      >
        <Icon name="grid" />
      </button>
    </div>
    <a href="#/jobs/new" class="btn btn-primary"><Icon name="plus" class="w-4 h-4" />{m.dashboard_new_job()}</a>
  {/snippet}
</PageHeader>

{#if !$jobsStore.loaded}
  <div class="flex justify-center py-24 text-primary-600"><Spinner size="w-7 h-7" /></div>
{:else if jobs.length === 0}
  <div class="card">
    <EmptyState icon="jobs" title={m.dashboard_no_jobs()} text={m.dashboard_create_first()}>
      {#snippet action()}
        <a href="#/jobs/new" class="btn btn-primary"><Icon name="plus" class="w-4 h-4" />{m.dashboard_new_job()}</a>
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <div class="flex flex-col lg:flex-row lg:items-center gap-3 mb-5">
    <div class="relative flex-1 lg:max-w-xs">
      <Icon name="search" class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none" />
      <input type="search" class="input pl-9" placeholder={m.jobs_search_placeholder()} bind:value={query} aria-label={m.common_search()} />
    </div>
    <div class="flex items-center gap-2 overflow-x-auto scrollbar-hide -mx-4 px-4 lg:mx-0 lg:px-0" role="group" aria-label={m.common_filter()}>
      <button
        type="button"
        class="chip h-8 px-3 border {filter === null ? 'bg-gray-900 text-white border-gray-900 dark:bg-white dark:text-gray-900 dark:border-white' : 'border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}"
        aria-pressed={filter === null}
        onclick={() => (filter = null)}>{m.common_all()} <span class="tabular-nums opacity-70">{jobs.length}</span></button
      >
      {#each FILTERS as f (f.id)}
        <button
          type="button"
          class="chip h-8 px-3 border {filter === f.id ? 'bg-gray-900 text-white border-gray-900 dark:bg-white dark:text-gray-900 dark:border-white' : 'border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}"
          aria-pressed={filter === f.id}
          onclick={() => (filter = filter === f.id ? null : f.id)}
          >{f.label()} <span class="tabular-nums opacity-70">{counts[f.id]}</span></button
        >
      {/each}
    </div>
    <div class="flex items-center gap-2 lg:ml-auto">
      <label for="job-sort" class="text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">{m.jobs_sort_by()}</label>
      <select
        id="job-sort"
        class="input h-9 w-auto text-sm"
        value={$sortStore.sortBy}
        onchange={(e) => sortStore.setSortBy(e.currentTarget.value as SortKey)}
      >
        <option value="status">{m.jobs_sort_status()}</option>
        <option value="name">{m.jobs_sort_name()}</option>
        <option value="last_success">{m.overview_last_good()}</option>
        <option value="next_run">{m.overview_next_run()}</option>
      </select>
      <button
        type="button"
        class="btn btn-ghost btn-icon btn-sm"
        onclick={() => sortStore.setSortOrder($sortStore.sortOrder === 'asc' ? 'desc' : 'asc')}
        aria-label={$sortStore.sortOrder === 'asc' ? m.jobs_sort_ascending() : m.jobs_sort_descending()}
        title={$sortStore.sortOrder === 'asc' ? m.jobs_sort_ascending() : m.jobs_sort_descending()}
      >
        <Icon name="arrows" class="w-4 h-4 {$sortStore.sortOrder === 'desc' ? 'rotate-180' : ''}" />
      </button>
    </div>
  </div>

  {#if visible.length === 0}
    <div class="card">
      <EmptyState icon="search" title={m.jobs_no_match()} compact>
        {#snippet action()}
          <button type="button" class="btn btn-secondary" onclick={() => ((query = ''), (filter = null))}>{m.jobs_clear_filters()}</button>
        {/snippet}
      </EmptyState>
    </div>
  {:else if $viewPreferencesStore === 'grid'}
    <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {#each visible as job (job.id)}
        <JobTile {job} runs={runs.get(job.id) ?? []} {now} />
      {/each}
    </div>
  {:else}
    <JobTable
      jobs={visible}
      {runs}
      {now}
      sort={{ by: $sortStore.sortBy, order: $sortStore.sortOrder }}
      onsort={(by) => sortStore.handleSort(by)}
    />
  {/if}
{/if}

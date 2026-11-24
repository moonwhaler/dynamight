<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import { showRunLog } from '../lib/stores/runLog';
  import type { JobRun } from '../lib/types';
  import { runDurationMs, runHasResult } from '../lib/utils/jobStatus';
  import { formatStatus } from '../lib/i18n/status';
  import { formatDuration, formatLocalizedDate } from '../lib/i18n/relativeTime';
  import { formatBytes } from '../lib/utils/format';
  import PageHeader from '../components/ui/PageHeader.svelte';
  import EmptyState from '../components/ui/EmptyState.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import Menu, { type MenuItem } from '../components/ui/Menu.svelte';
  import { confirm } from '../components/ui/ConfirmDialog.svelte';
  import { showToast } from '../components/ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  const PAGE = 50;
  const STATUSES = ['completed', 'failed', 'cancelled', 'running'] as const;

  let runs = $state<JobRun[]>([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let hasMore = $state(false);
  let generation = 0;

  let query = $state('');
  let jobId = $state<number | null>(null);
  let statuses = $state<Set<string>>(new Set());
  let from = $state('');
  let to = $state('');

  onMount(() => jobsStore.live());

  const jobName = (id: number) => $jobsStore.jobs.find((j) => j.id === id)?.name ?? `#${id}`;

  function serverQuery() {
    const f = from ? new Date(from) : null;
    f?.setHours(0, 0, 0, 0);
    const t = to ? new Date(to) : null;
    t?.setHours(23, 59, 59, 999);
    return {
      jobId: jobId ?? undefined,
      status: statuses.size ? [...statuses] : undefined,
      from: f?.toISOString(),
      to: t?.toISOString(),
    };
  }

  /** Newest page; a silent reload keeps older pages already loaded */
  async function load(silent = false) {
    const gen = ++generation;
    if (!silent) loading = true;
    try {
      const page = await api.runs.recent({ ...serverQuery(), limit: PAGE });
      if (gen !== generation) return;
      if (silent && runs.length > PAGE && page.length === PAGE) {
        const oldest = page[page.length - 1].id;
        runs = [...page, ...runs.filter((r) => r.id < oldest)];
      } else {
        runs = page;
        hasMore = page.length === PAGE;
      }
    } catch (e) {
      if (!silent) showToast({ message: e instanceof Error ? e.message : m.error_failed_load(), variant: 'error' });
    } finally {
      if (!silent && gen === generation) loading = false;
    }
  }

  async function loadMore() {
    if (loadingMore || runs.length === 0) return;
    const gen = generation;
    loadingMore = true;
    try {
      const page = await api.runs.recent({ ...serverQuery(), limit: PAGE, before: runs[runs.length - 1].id });
      if (gen !== generation) return;
      runs = [...runs, ...page];
      hasMore = page.length === PAGE;
    } finally {
      loadingMore = false;
    }
  }

  // Server-side filters: reload when one changes (also the first load)
  $effect(() => {
    void [jobId, statuses, from, to];
    untrack(() => load());
  });

  // Runs started or finished elsewhere: refresh quietly
  let seenRuns: unknown = null;
  $effect(() => {
    const r = $jobsStore.runs;
    if (seenRuns !== null && r !== seenRuns) untrack(() => load(true));
    seenRuns = r;
  });

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? runs.filter((r) => jobName(r.job_id).toLowerCase().includes(q) || (r.summary ?? '').toLowerCase().includes(q)) : runs;
  });

  /** Runs grouped by the day they started, newest day first */
  const days = $derived.by(() => {
    const groups: { key: string; label: string; runs: JobRun[] }[] = [];
    const today = new Date();
    const yesterday = new Date(Date.now() - 86_400_000);
    for (const run of shown) {
      const d = run.started_at ? new Date(run.started_at) : null;
      const key = d ? d.toDateString() : '—';
      let group = groups[groups.length - 1];
      if (!group || group.key !== key) {
        const label = !d
          ? '—'
          : key === today.toDateString()
            ? m.activity_today()
            : key === yesterday.toDateString()
              ? m.activity_yesterday()
              : d.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: d.getFullYear() === today.getFullYear() ? undefined : 'numeric' });
        group = { key, label, runs: [] };
        groups.push(group);
      }
      group.runs.push(run);
    }
    return groups;
  });

  const filterCount = $derived((jobId ? 1 : 0) + (statuses.size ? 1 : 0) + (from ? 1 : 0) + (to ? 1 : 0) + (query.trim() ? 1 : 0));

  function toggleStatus(s: string) {
    const next = new Set(statuses);
    if (next.has(s)) next.delete(s);
    else next.add(s);
    statuses = next;
  }

  function clearFilters() {
    query = '';
    jobId = null;
    statuses = new Set();
    from = '';
    to = '';
  }

  async function deleteRun(run: JobRun) {
    const ok = await confirm({
      title: m.history_confirm_delete_title(),
      message: m.history_purge_run_confirm(),
      confirmText: m.common_delete(),
      variant: 'danger',
    });
    if (!ok) return;
    try {
      await api.runs.delete(run.id);
      runs = runs.filter((r) => r.id !== run.id);
      jobsStore.refresh();
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    }
  }

  async function deleteForJob(id: number) {
    const ok = await confirm({
      title: m.history_confirm_delete_title(),
      message: m.history_purge_job_confirm({ name: jobName(id) }),
      confirmText: m.common_delete(),
      variant: 'danger',
    });
    if (!ok) return;
    try {
      await api.runs.deleteForJob(id);
      await load();
      jobsStore.refresh();
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    }
  }

  async function deleteAll() {
    const word = m.activity_delete_word();
    const ok = await confirm({
      title: m.activity_delete_all_title(),
      message: m.history_purge_all_confirm(),
      confirmText: m.activity_delete_all(),
      variant: 'danger',
      requireText: word,
    });
    if (!ok) return;
    try {
      await api.runs.purgeAll();
      await load();
      jobsStore.refresh();
      showToast({ message: m.activity_deleted_all(), variant: 'success' });
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    }
  }

  const pageMenu = $derived<MenuItem[]>([
    ...(jobId ? [{ label: m.activity_delete_job({ name: jobName(jobId) }), icon: 'trash' as const, danger: true, onclick: () => deleteForJob(jobId!) }] : []),
    { label: m.activity_delete_all_menu(), icon: 'trash', danger: true, onclick: deleteAll, disabled: runs.length === 0 && filterCount === 0 },
  ]);

  function rowMenu(run: JobRun): MenuItem[] {
    return [
      { label: m.dashboard_open_log(), icon: 'log', onclick: () => showRunLog(run.id, run.job_id) },
      { label: m.jobs_menu_open(), icon: 'eye', href: `#/jobs/${run.job_id}` },
      'separator',
      { label: m.activity_delete_run(), icon: 'trash', danger: true, onclick: () => deleteRun(run), disabled: run.status === 'running' || run.status === 'pending' },
    ];
  }

  const tone: Record<JobRun['status'], string> = {
    completed: 'text-green-600 dark:text-green-400',
    failed: 'text-red-600 dark:text-red-400',
    cancelled: 'text-amber-600 dark:text-amber-400',
    running: 'text-primary-600 dark:text-primary-400',
    pending: 'text-primary-600 dark:text-primary-400',
  };
  const icon = { completed: 'check-circle', failed: 'x-circle', cancelled: 'pause', running: 'refresh', pending: 'clock' } as const;

  function time(ts: string | null): string {
    return ts ? new Date(ts).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }) : '';
  }
</script>

<PageHeader title={m.nav_activity()} subtitle={m.activity_subtitle()}>
  {#snippet actions()}
    <Menu label={m.common_actions()} items={pageMenu} triggerClass="btn btn-secondary btn-icon" />
  {/snippet}
</PageHeader>

<!-- Filters -->
<div class="card p-3 sm:p-4 mb-6 space-y-3">
  <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-[1.4fr_1fr_auto_auto]">
    <div class="relative">
      <Icon name="search" class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none" />
      <input type="search" class="input pl-9" placeholder={m.activity_search()} aria-label={m.common_search()} bind:value={query} />
    </div>
    <select class="input" aria-label={m.history_table_job()} bind:value={jobId}>
      <option value={null}>{m.activity_all_jobs()}</option>
      {#each $jobsStore.jobs as j (j.id)}
        <option value={j.id}>{j.name}</option>
      {/each}
    </select>
    <label class="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
      <span class="shrink-0">{m.activity_from()}</span>
      <input type="date" class="input" bind:value={from} max={to || undefined} />
    </label>
    <label class="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
      <span class="shrink-0">{m.activity_to()}</span>
      <input type="date" class="input" bind:value={to} min={from || undefined} />
    </label>
  </div>
  <div class="flex flex-wrap items-center gap-2">
    {#each STATUSES as s (s)}
      <button
        type="button"
        class="chip h-8 px-3 border {statuses.has(s)
          ? 'bg-gray-900 text-white border-gray-900 dark:bg-white dark:text-gray-900 dark:border-white'
          : 'border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}"
        aria-pressed={statuses.has(s)}
        onclick={() => toggleStatus(s)}
      >
        <Icon name={icon[s]} class="w-3.5 h-3.5 {statuses.has(s) ? '' : tone[s]}" />{formatStatus(s)}
      </button>
    {/each}
    {#if filterCount > 0}
      <button type="button" class="btn btn-ghost btn-sm ml-auto" onclick={clearFilters}>{m.jobs_clear_filters()}</button>
    {/if}
  </div>
</div>

{#if loading}
  <div class="flex justify-center py-20 text-primary-600"><Spinner size="w-7 h-7" /></div>
{:else if shown.length === 0}
  <div class="card">
    <EmptyState icon="clock" title={filterCount ? m.activity_no_match() : m.dashboard_no_recent_runs()} compact>
      {#snippet action()}
        {#if filterCount}<button type="button" class="btn btn-secondary" onclick={clearFilters}>{m.jobs_clear_filters()}</button>{/if}
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <!-- Phones: one card per day -->
  <div class="md:hidden space-y-6">
    {#each days as day (day.key)}
      <section aria-label={day.label}>
        <h2 class="eyebrow mb-2">{day.label}</h2>
        <ul class="card rows overflow-hidden">
          {#each day.runs as run (run.id)}
            {@const d = runDurationMs(run)}
            {@const result = runHasResult(run)}
            <li class="relative flex items-start gap-3 px-4 py-3">
              <span class="mt-0.5 shrink-0 {tone[run.status]}" title={formatStatus(run.status)}>
                <Icon name={icon[run.status]} class="w-5 h-5 {run.status === 'running' ? 'animate-spin [animation-duration:2.5s]' : ''}" />
              </span>
              <button type="button" class="flex-1 min-w-0 text-left after:absolute after:inset-0" onclick={() => showRunLog(run.id, run.job_id)}>
                <span class="flex items-baseline gap-2 min-w-0">
                  <span class="font-medium text-gray-900 dark:text-white truncate">{jobName(run.job_id)}</span>
                  <span class="text-xs text-gray-500 dark:text-gray-400 shrink-0">{formatStatus(run.status)}</span>
                </span>
                {#if run.summary && run.status !== 'completed'}
                  <span class="block text-xs font-mono truncate mt-0.5 {run.status === 'failed' ? 'text-red-600 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'}">{run.summary}</span>
                {/if}
                <span class="block text-xs text-gray-500 dark:text-gray-400 tabular-nums mt-0.5">
                  {time(run.started_at)}{#if d != null} · {formatDuration(d)}{/if}{#if result && run.bytes_transferred} · {formatBytes(run.bytes_transferred)}{/if}
                </span>
              </button>
              <div class="relative z-10 -mr-2 shrink-0">
                <Menu label={m.common_actions()} items={rowMenu(run)} triggerClass="btn btn-ghost btn-icon btn-sm" />
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  <!-- Tablets and desktop: one table, a row per day as separator -->
  <div class="hidden md:block card overflow-hidden">
    <table class="table">
      <thead>
        <tr>
          <th>{m.history_table_job()}</th>
          <th>{m.jobs_sort_status()}</th>
          <th class="text-right">{m.history_table_started()}</th>
          <th class="text-right">{m.history_table_duration()}</th>
          <th class="text-right">{m.history_table_size()}</th>
          <th class="text-right hidden lg:table-cell">{m.history_table_files()}</th>
          <th class="w-px"><span class="sr-only">{m.common_actions()}</span></th>
        </tr>
      </thead>
      {#each days as day (day.key)}
        <tbody>
          <tr class="hover:bg-transparent dark:hover:bg-transparent">
            <th colspan="7" scope="rowgroup" class="px-4 pt-4 pb-1.5 text-left text-xs font-semibold text-gray-500 dark:text-gray-400 border-b border-gray-100 dark:border-gray-700/60">
              {day.label}
            </th>
          </tr>
          {#each day.runs as run (run.id)}
            {@const d = runDurationMs(run)}
            {@const result = runHasResult(run)}
            <tr class="cursor-pointer" onclick={() => showRunLog(run.id, run.job_id)}>
              <td class="max-w-0 w-[45%]">
                <button type="button" class="font-medium text-gray-900 dark:text-white hover:underline truncate block max-w-full text-left" onclick={(e) => (e.stopPropagation(), showRunLog(run.id, run.job_id))}>
                  {jobName(run.job_id)}
                </button>
                {#if run.summary && run.status !== 'completed'}
                  <span class="block text-xs font-mono truncate mt-0.5 {run.status === 'failed' ? 'text-red-600 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'}" title={run.summary}>{run.summary}</span>
                {/if}
              </td>
              <td class="whitespace-nowrap">
                <span class="inline-flex items-center gap-1.5 {tone[run.status]}">
                  <Icon name={icon[run.status]} class="w-4 h-4 {run.status === 'running' ? 'animate-spin [animation-duration:2.5s]' : ''}" />
                  <span class="text-gray-700 dark:text-gray-300">{formatStatus(run.status)}</span>
                </span>
              </td>
              <td class="text-right whitespace-nowrap tabular-nums text-gray-700 dark:text-gray-300" title={run.started_at ? formatLocalizedDate(new Date(run.started_at)) : ''}>{time(run.started_at)}</td>
              <td class="text-right whitespace-nowrap tabular-nums text-gray-700 dark:text-gray-300">{d != null ? formatDuration(d) : '—'}</td>
              <td class="text-right whitespace-nowrap tabular-nums text-gray-700 dark:text-gray-300">{result ? formatBytes(run.bytes_transferred ?? 0) : '—'}</td>
              <td class="text-right whitespace-nowrap tabular-nums text-gray-500 dark:text-gray-400 hidden lg:table-cell">{result ? (run.files_transferred ?? 0) : '—'}</td>
              <td class="whitespace-nowrap">
                <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                <div class="flex justify-end" onclick={(e) => e.stopPropagation()}>
                  <Menu label={m.common_actions()} items={rowMenu(run)} triggerClass="btn btn-ghost btn-icon btn-sm" />
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      {/each}
    </table>
  </div>

  {#if hasMore}
    <div class="flex justify-center mt-6">
      <button type="button" class="btn btn-secondary" onclick={loadMore} disabled={loadingMore}>
        {#if loadingMore}<Spinner />{/if}{m.jobview_load_more()}
      </button>
    </div>
  {/if}
{/if}

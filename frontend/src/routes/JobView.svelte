<script lang="ts">
  import { onMount } from 'svelte';
  import { location } from 'svelte-spa-router';
  import { jobsStore } from '../lib/stores/jobs';
  import { api } from '../lib/api';
  import type { JobRun } from '../lib/types';
  import { jobStatus, runDurationMs } from '../lib/utils/jobStatus';
  import { destinationKind, destinationTarget } from '../lib/utils/jobUtils';
  import { DESTINATION_ICON, destinationKindLabel } from '../lib/utils/destination';
  import { formatBytes } from '../lib/utils/format';
  import { formatRelativeTime, formatDateTime, formatDuration, formatTimeUntil } from '../lib/i18n/relativeTime';
  import { formatStatus } from '../lib/i18n/status';
  import { formatCron } from '../lib/i18n/schedule';
  import { jobSummary } from '../lib/i18n/jobSummary';
  import { formatScheduleTime, serverTimezone } from '../lib/stores/serverTimezone';
  import { isRunning, runJob, stopJob } from '../lib/jobActions';
  import { showRunLog } from '../lib/stores/runLog';
  import StatusChip from '../components/ui/StatusChip.svelte';
  import RunStrip from '../components/ui/RunStrip.svelte';
  import PathText from '../components/ui/PathText.svelte';
  import Tabs from '../components/ui/Tabs.svelte';
  import Menu from '../components/ui/Menu.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import EmptyState from '../components/ui/EmptyState.svelte';
  import RunItem from '../components/runs/RunItem.svelte';
  import { jobMenuItems } from '../components/jobs/jobMenu';
  import EditJob from '../components/jobs/editor/EditJob.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { params = {} }: { params?: { id?: string } } = $props();

  const id = $derived(Number(params.id));
  const tab = $derived($location.endsWith('/runs') ? 'runs' : $location.endsWith('/settings') ? 'settings' : 'overview');

  const job = $derived($jobsStore.jobs.find((j) => j.id === id) ?? null);
  let runs = $state<JobRun[]>([]);
  let runsLoaded = $state(false);
  let moreRuns = $state(true);
  let loadingMore = $state(false);
  let busy = $state(false);
  let now = $state(Date.now());

  const PAGE = 50;

  async function loadRuns() {
    try {
      const list = await api.runs.list(id, PAGE, 0);
      // Keep extra pages the user loaded
      runs = runs.length > PAGE ? [...list, ...runs.slice(list.length)] : list;
      moreRuns = list.length === PAGE;
    } catch {
      // The job may be gone; the page shows that
    } finally {
      runsLoaded = true;
    }
  }

  async function loadMore() {
    loadingMore = true;
    try {
      const list = await api.runs.list(id, PAGE, runs.length);
      runs = [...runs, ...list];
      moreRuns = list.length === PAGE;
    } finally {
      loadingMore = false;
    }
  }

  onMount(() => {
    const stop = jobsStore.live();
    const clock = setInterval(() => (now = Date.now()), 60_000);
    return () => {
      stop();
      clearInterval(clock);
    };
  });

  // Reload this job's runs whenever the shared run list changes (start/finish)
  $effect(() => {
    void $jobsStore.runs;
    if (id) loadRuns();
  });

  $effect(() => {
    if (job) document.title = `${job.name} · Dynamight`;
  });

  const status = $derived(job ? jobStatus(job, now) : null);
  const running = $derived(job ? isRunning(job) : false);
  const lastRun = $derived(runs[0] ?? null);
  const kind = $derived(job ? destinationKind(job) : 'local');
  const schedules = $derived(job?.schedules ?? []);
  const used = $derived(
    job?.dest_storage_total && job.dest_storage_free != null
      ? Math.round(((job.dest_storage_total - job.dest_storage_free) / job.dest_storage_total) * 100)
      : null,
  );

  async function toggleRun() {
    if (!job) return;
    busy = true;
    if (running) await stopJob(job);
    else await runJob(job);
    busy = false;
  }
</script>

{#if !$jobsStore.loaded}
  <div class="flex justify-center py-24 text-primary-600"><Spinner size="w-7 h-7" /></div>
{:else if !job}
  <div class="card">
    <EmptyState icon="jobs" title={m.jobview_not_found()}>
      {#snippet action()}<a href="#/jobs" class="btn btn-secondary">{m.job_back_to_jobs()}</a>{/snippet}
    </EmptyState>
  </div>
{:else}
  <a href="#/jobs" class="inline-flex items-center gap-1 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white mb-3">
    <Icon name="chevron-left" class="w-4 h-4" />{m.jobview_back()}
  </a>

  <header class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between mb-5">
    <div class="min-w-0">
      <h1 class="font-display text-2xl sm:text-[2rem] leading-tight text-gray-900 dark:text-white break-words">{job.name}</h1>
      <div class="flex flex-wrap items-center gap-2 mt-2">
        {#if status}<StatusChip {status} hasRun={!!job.last_run_status} />{/if}
        {#if job.sync_options?.dry_run}<span class="chip badge-warning">{m.jobview_dry_run()}</span>{/if}
        {#if job.description}<span class="text-sm text-gray-500 dark:text-gray-400">{job.description}</span>{/if}
      </div>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      <button
        type="button"
        class="btn {running ? 'btn-danger' : 'btn-primary'}"
        onclick={toggleRun}
        disabled={busy || (!job.enabled && !running)}
        title={!job.enabled && !running ? m.job_card_tooltip_enable() : undefined}
      >
        {#if busy}<Spinner />{:else}<Icon name={running ? 'stop' : 'play'} class="w-4 h-4" solid={!running} />{/if}
        {running ? m.job_btn_stop() : m.overview_run_now()}
      </button>
      <Menu label={m.common_actions()} items={jobMenuItems(job, { withOpen: false })} triggerClass="btn btn-secondary btn-icon" />
    </div>
  </header>

  <div class="mb-6">
    <Tabs
      active={tab}
      tabs={[
        { id: 'overview', label: m.jobview_tab_overview(), href: `#/jobs/${id}` },
        { id: 'runs', label: m.jobview_tab_runs(), href: `#/jobs/${id}/runs` },
        { id: 'settings', label: m.jobview_tab_settings(), href: `#/jobs/${id}/settings` },
      ]}
    />
  </div>

  {#if tab === 'overview'}
    <p class="text-[15px] leading-relaxed text-gray-700 dark:text-gray-300 max-w-3xl mb-6">{jobSummary(job, schedules)}</p>

    <div class="grid gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)] items-start">
      <section class="card overflow-hidden" aria-labelledby="runs-title">
        <div class="px-4 sm:px-5 pt-4 pb-4 border-b border-gray-200 dark:border-gray-700">
          <div class="flex items-baseline justify-between mb-3">
            <h2 id="runs-title" class="eyebrow">{m.jobview_recent_runs()}</h2>
            <a href="#/jobs/{id}/runs" class="text-sm text-primary-700 dark:text-primary-400 hover:underline">{m.jobview_all_runs()}</a>
          </div>
          {#if runs.length > 0}
            <RunStrip {runs} slots={40} height="h-16" barClass="w-2.5 sm:w-3 shrink-0" onselect={(r) => showRunLog(r.id, id)} />
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">{m.jobview_strip_legend()}</p>
          {:else if runsLoaded}
            <p class="text-sm text-gray-500 dark:text-gray-400">{m.overview_no_runs()}</p>
          {/if}
        </div>
        {#if runs.length > 0}
          <ul class="rows">
            {#each runs.slice(0, 5) as run (run.id)}
              <li><RunItem {run} showJob={false} /></li>
            {/each}
          </ul>
        {/if}
      </section>

      <div class="space-y-6">
        <section class="card p-4 sm:p-5">
          <dl class="grid grid-cols-2 gap-4 text-sm">
            <div>
              <dt class="text-xs text-gray-500 dark:text-gray-400">{m.overview_last_good()}</dt>
              <dd class="mt-0.5 font-medium text-gray-900 dark:text-white tabular-nums" title={status?.lastSuccessAt ? formatDateTime(status.lastSuccessAt) : ''}>
                {status?.lastSuccessAt ? formatRelativeTime(status.lastSuccessAt) : m.common_never()}
              </dd>
            </div>
            <div>
              <dt class="text-xs text-gray-500 dark:text-gray-400">{m.jobview_last_run()}</dt>
              <dd class="mt-0.5 font-medium text-gray-900 dark:text-white tabular-nums">
                {#if lastRun}
                  {formatStatus(lastRun.status)}
                  <span class="font-normal text-gray-500 dark:text-gray-400">
                    · {formatRelativeTime(lastRun.started_at)}{#if runDurationMs(lastRun) != null} · {formatDuration(runDurationMs(lastRun)!)}{/if}
                  </span>
                {:else}
                  —
                {/if}
              </dd>
            </div>
          </dl>
        </section>

        <section class="card p-4 sm:p-5" aria-labelledby="sched-title">
          <div class="flex items-baseline justify-between mb-3">
            <h2 id="sched-title" class="eyebrow">{m.jobview_schedules()}</h2>
            <a href="#/jobs/{id}/settings?section=schedule" class="text-sm text-primary-700 dark:text-primary-400 hover:underline">{m.common_edit()}</a>
          </div>
          {#if schedules.length === 0}
            <p class="text-sm text-gray-500 dark:text-gray-400">{m.jobview_no_schedules()}</p>
          {:else}
            <ul class="space-y-2.5">
              {#each schedules as s (s.id)}
                <li class="flex items-baseline justify-between gap-3 text-sm">
                  <span class="text-gray-900 dark:text-white {s.enabled ? '' : 'line-through opacity-60'}">{formatCron(s.cron_expression)}</span>
                  <span class="text-right text-gray-500 dark:text-gray-400 tabular-nums shrink-0">
                    {#if !s.enabled || !job.enabled}
                      {m.jobview_paused()}
                    {:else if s.next_run_at}
                      <span title={formatScheduleTime(s.next_run_at, $serverTimezone)}>{formatTimeUntil(s.next_run_at)}</span>
                    {/if}
                  </span>
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        <section class="card p-4 sm:p-5 space-y-4">
          <div>
            <h2 class="eyebrow mb-2">{m.jobview_sources()}</h2>
            <ul class="space-y-1.5 text-sm text-gray-800 dark:text-gray-200">
              {#each job.source_dirs as dir (dir)}
                <li class="flex items-center gap-2 min-w-0">
                  <Icon name="folder" class="w-4 h-4 text-gray-400 shrink-0" /><PathText path={dir} max={44} class="truncate" />
                </li>
              {/each}
            </ul>
          </div>
          <div>
            <h2 class="eyebrow mb-2">{m.jobview_destination()}</h2>
            <p class="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200 min-w-0">
              <Icon name={DESTINATION_ICON[kind]} class="w-4 h-4 text-gray-400 shrink-0" />
              <span class="shrink-0">{destinationKindLabel(kind)}</span>
              <PathText path={destinationTarget(job)} max={36} class="truncate text-gray-500 dark:text-gray-400" />
            </p>
            {#if used != null}
              <div class="mt-3">
                <div class="h-2 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                  <div class="h-full rounded-full {used > 90 ? 'bg-red-500' : used > 75 ? 'bg-amber-500' : 'bg-gray-500 dark:bg-gray-400'}" style="width:{used}%"></div>
                </div>
                <p class="text-xs text-gray-500 dark:text-gray-400 mt-1 tabular-nums">
                  {m.storage_tooltip({ free: formatBytes(job.dest_storage_free ?? 0), total: formatBytes(job.dest_storage_total) })}
                </p>
              </div>
            {/if}
          </div>
        </section>
      </div>
    </div>
  {:else if tab === 'runs'}
    {#if !runsLoaded}
      <div class="flex justify-center py-16 text-primary-600"><Spinner size="w-6 h-6" /></div>
    {:else if runs.length === 0}
      <div class="card"><EmptyState icon="clock" title={m.overview_no_runs()} compact /></div>
    {:else}
      <ul class="card rows overflow-hidden">
        {#each runs as run (run.id)}
          <li><RunItem {run} showJob={false} /></li>
        {/each}
      </ul>
      {#if moreRuns}
        <div class="flex justify-center mt-4">
          <button type="button" class="btn btn-secondary" onclick={loadMore} disabled={loadingMore}>
            {#if loadingMore}<Spinner />{/if}{m.jobview_load_more()}
          </button>
        </div>
      {/if}
    {/if}
  {:else}
    {#key id}<EditJob jobId={id} />{/key}
  {/if}
{/if}

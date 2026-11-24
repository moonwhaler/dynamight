<script lang="ts">
  import { onMount } from 'svelte';
  import { jobsStore } from '../lib/stores/jobs';
  import { api } from '../lib/api';
  import { jobStatus, STATE_ORDER, upcomingRuns, runsByJob, type JobStatus } from '../lib/utils/jobStatus';
  import { oldestBackup } from '../lib/utils/backupHealth';
  import { formatRelativeTime, formatTimeUntil, formatDateTime } from '../lib/i18n/relativeTime';
  import { formatScheduleTime, serverTimezone } from '../lib/stores/serverTimezone';
  import { runJob, openLatestLog } from '../lib/jobActions';
  import type { Job } from '../lib/types';
  import JobTable from '../components/jobs/JobTable.svelte';
  import RunItem from '../components/runs/RunItem.svelte';
  import StatusChip from '../components/ui/StatusChip.svelte';
  import EmptyState from '../components/ui/EmptyState.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';

  /** Jobs listed on the overview; the rest is one click away */
  const OVERVIEW_JOBS = 8;

  // Ticks every minute so "overdue" and relative times stay current
  let now = $state(Date.now());
  let notificationsOff = $state(false);

  onMount(() => {
    const stop = jobsStore.live();
    const clock = setInterval(() => (now = Date.now()), 60_000);
    api.notifications
      .get()
      .then((n) => (notificationsOff = !n.ntfy.enabled && !n.webhook.enabled))
      .catch(() => {});
    return () => {
      stop();
      clearInterval(clock);
    };
  });

  $effect(() => {
    document.title = `${m.nav_overview()} · Dynamight`;
  });

  const jobs = $derived($jobsStore.jobs);
  const statuses = $derived(new Map<number, JobStatus>(jobs.map((j) => [j.id, jobStatus(j, now)])));
  const attention = $derived(
    jobs
      .filter((j) => statuses.get(j.id)?.attention)
      .sort((a, b) => STATE_ORDER[statuses.get(a.id)!.state] - STATE_ORDER[statuses.get(b.id)!.state]),
  );
  const running = $derived(jobs.filter((j) => statuses.get(j.id)?.state === 'running'));
  const sorted = $derived(
    [...jobs].sort(
      (a, b) =>
        STATE_ORDER[statuses.get(a.id)!.state] - STATE_ORDER[statuses.get(b.id)!.state] || a.name.localeCompare(b.name),
    ),
  );
  const runs = $derived(runsByJob($jobsStore.runs));
  const recent = $derived($jobsStore.runs.slice(0, 8));
  const upcoming = $derived(upcomingRuns(jobs).slice(0, 6));
  const oldest = $derived(oldestBackup(jobs));
  const jobName = (id: number) => jobs.find((j) => j.id === id)?.name;

  const today = $derived(
    new Date(now).toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long' }),
  );

  function reason(job: Job, s: JobStatus): string {
    switch (s.state) {
      case 'failed': {
        const last = runs.get(job.id)?.[0];
        return last?.started_at
          ? m.overview_reason_failed({ when: formatRelativeTime(last.started_at) })
          : m.dashboard_attention_failed();
      }
      case 'overdue':
        return m.overview_reason_overdue({ when: formatRelativeTime(s.lastSuccessAt) });
      case 'never':
        return m.overview_reason_never({ when: formatRelativeTime(job.created_at) });
      default:
        return '';
    }
  }

  function failureNote(job: Job): string | null {
    const last = runs.get(job.id)?.[0];
    return last?.status === 'failed' && typeof last.summary === 'string' && last.summary ? last.summary : null;
  }
</script>

{#if !$jobsStore.loaded}
  <div class="flex justify-center py-24 text-primary-600"><Spinner size="w-7 h-7" /></div>
{:else if $jobsStore.error && jobs.length === 0}
  <div class="card p-8 text-center space-y-4">
    <p class="text-red-600 dark:text-red-400">{$jobsStore.error}</p>
    <button type="button" class="btn btn-secondary" onclick={() => jobsStore.load()}>{m.common_retry()}</button>
  </div>
{:else if jobs.length === 0}
  <p class="eyebrow mb-2">{today}</p>
  <h1 class="font-display text-3xl sm:text-4xl text-gray-900 dark:text-white mb-8">{m.overview_headline_empty()}</h1>
  <div class="card">
    <EmptyState icon="archive" title={m.dashboard_no_jobs()} text={m.dashboard_create_first()}>
      {#snippet action()}
        <a href="#/jobs/new" class="btn btn-primary"><Icon name="plus" class="w-4 h-4" />{m.dashboard_new_job()}</a>
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <!-- Headline: the one answer this page exists for -->
  <section class="mb-8 lg:mb-10">
    <div class="flex items-start justify-between gap-4">
      <div class="min-w-0">
        <p class="eyebrow mb-2">{today}</p>
        <h1 class="font-display text-[1.6rem] sm:text-[2.1rem] leading-[1.12] text-gray-900 dark:text-white max-w-3xl">
          {#if attention.length > 0}
            <span class="inline-block w-2.5 h-2.5 sm:w-3 sm:h-3 rounded-full bg-red-500 align-middle mr-2 sm:mr-3 -mt-1" aria-hidden="true"></span>{m.overview_headline_attention({ count: attention.length, total: jobs.length })}
          {:else if running.length > 0}
            {m.overview_headline_running({ name: running[0].name })}
          {:else}
            <span class="inline-block w-2.5 h-2.5 sm:w-3 sm:h-3 rounded-full bg-green-500 align-middle mr-2 sm:mr-3 -mt-1" aria-hidden="true"></span>{m.overview_headline_ok()}
          {/if}
        </h1>
      </div>
      <a href="#/jobs/new" class="hidden sm:inline-flex btn btn-primary shrink-0 mt-6">
        <Icon name="plus" class="w-4 h-4" />{m.dashboard_new_job()}
      </a>
    </div>
    <dl class="mt-4 flex flex-wrap gap-x-8 gap-y-2 text-sm">
      {#if oldest}
        <div class="flex gap-2">
          <dt class="text-gray-500 dark:text-gray-400">{m.overview_oldest_copy()}</dt>
          <dd class="text-gray-900 dark:text-white font-medium tabular-nums">
            {oldest.at ? formatRelativeTime(oldest.at) : m.common_never()}
            <a href="#/jobs/{oldest.job.id}" class="font-normal text-gray-500 dark:text-gray-400 hover:underline">({oldest.job.name})</a>
          </dd>
        </div>
      {/if}
      {#if upcoming[0]}
        <div class="flex gap-2">
          <dt class="text-gray-500 dark:text-gray-400">{m.overview_next_run()}</dt>
          <dd class="text-gray-900 dark:text-white font-medium tabular-nums" title={formatDateTime(upcoming[0].at)}>
            {formatTimeUntil(upcoming[0].at)}
            <a href="#/jobs/{upcoming[0].job.id}" class="font-normal text-gray-500 dark:text-gray-400 hover:underline">({upcoming[0].job.name})</a>
          </dd>
        </div>
      {/if}
      <div class="flex gap-2">
        <dt class="text-gray-500 dark:text-gray-400">{m.overview_jobs_active()}</dt>
        <dd class="text-gray-900 dark:text-white font-medium tabular-nums">
          {jobs.filter((j) => j.enabled).length}/{jobs.length}
        </dd>
      </div>
    </dl>
  </section>

  {#if attention.length > 0}
    <section class="mb-8" aria-labelledby="attention-title">
      <h2 id="attention-title" class="eyebrow mb-3 text-red-600 dark:text-red-400">{m.dashboard_attention()}</h2>
      <ul class="card rows overflow-hidden border-red-200 dark:border-red-900/60">
        {#each attention as job (job.id)}
          {@const s = statuses.get(job.id)!}
          {@const note = failureNote(job)}
          <li class="flex flex-col sm:flex-row sm:items-center gap-3 px-4 sm:px-5 py-3.5">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <a href="#/jobs/{job.id}" class="font-medium text-gray-900 dark:text-white hover:underline truncate">{job.name}</a>
                <StatusChip status={s} size="sm" />
              </div>
              <p class="text-sm text-gray-600 dark:text-gray-300 mt-0.5">{reason(job, s)}</p>
              {#if note}<p class="text-xs font-mono text-red-600 dark:text-red-400 mt-1 truncate" title={note}>{note}</p>{/if}
            </div>
            <div class="flex gap-2 shrink-0">
              {#if runs.get(job.id)?.length}
                <button type="button" class="btn btn-ghost btn-sm" onclick={() => openLatestLog(job)}>
                  <Icon name="log" class="w-4 h-4" />{m.dashboard_open_log()}
                </button>
              {/if}
              {#if job.enabled}
                <button type="button" class="btn btn-secondary btn-sm" onclick={() => runJob(job)}>
                  <Icon name="play" class="w-4 h-4" solid />{m.overview_run_now()}
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="mb-8" aria-labelledby="jobs-title">
    <div class="flex items-baseline justify-between mb-3">
      <h2 id="jobs-title" class="eyebrow">{m.nav_jobs()}</h2>
      <a href="#/jobs" class="text-sm text-primary-700 dark:text-primary-400 hover:underline">{m.overview_all_jobs()}</a>
    </div>
    <JobTable jobs={sorted.slice(0, OVERVIEW_JOBS)} {runs} {now} />
    {#if sorted.length > OVERVIEW_JOBS}
      <a href="#/jobs" class="mt-3 inline-flex items-center gap-1 text-sm text-primary-700 dark:text-primary-400 hover:underline">
        {m.overview_more_jobs({ count: sorted.length - OVERVIEW_JOBS })}<Icon name="chevron-right" class="w-4 h-4" />
      </a>
    {/if}
  </section>

  <div class="grid gap-8 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
    <section aria-labelledby="upcoming-title">
      <h2 id="upcoming-title" class="eyebrow mb-3">{m.overview_coming_up()}</h2>
      {#if upcoming.length === 0}
        <div class="card"><EmptyState icon="calendar" title={m.dashboard_no_schedules()} compact /></div>
      {:else}
        <ul class="card rows overflow-hidden">
          {#each upcoming as u (u.scheduleId)}
            <li class="flex items-center justify-between gap-3 px-4 sm:px-5 py-3">
              <a href="#/jobs/{u.job.id}" class="font-medium text-gray-900 dark:text-white hover:underline truncate">{u.job.name}</a>
              <span class="text-right shrink-0">
                <span class="block text-sm text-gray-900 dark:text-white tabular-nums">{formatTimeUntil(u.at)}</span>
                <span class="block text-xs text-gray-500 dark:text-gray-400 tabular-nums">{formatScheduleTime(u.at, $serverTimezone)}</span>
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="recent-title">
      <div class="flex items-baseline justify-between mb-3">
        <h2 id="recent-title" class="eyebrow">{m.dashboard_recent_activity()}</h2>
        <a href="#/activity" class="text-sm text-primary-700 dark:text-primary-400 hover:underline">{m.overview_all_activity()}</a>
      </div>
      {#if recent.length === 0}
        <div class="card"><EmptyState icon="clock" title={m.dashboard_no_recent_runs()} compact /></div>
      {:else}
        <ul class="card rows overflow-hidden">
          {#each recent as run (run.id)}
            <li><RunItem {run} jobName={jobName(run.job_id)} /></li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>

  {#if notificationsOff}
    <a
      href="#/settings/notifications"
      class="mt-8 flex items-center gap-3 p-4 rounded-xl border border-dashed border-gray-300 dark:border-gray-700 text-sm text-gray-600 dark:text-gray-300 hover:bg-white dark:hover:bg-gray-800 transition-colors"
    >
      <Icon name="bell" class="w-5 h-5 text-gray-400 shrink-0" />
      <span class="flex-1">{m.overview_notifications_hint()}</span>
      <Icon name="chevron-right" class="w-4 h-4 text-gray-400" />
    </a>
  {/if}
{/if}

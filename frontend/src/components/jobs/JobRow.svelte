<script lang="ts">
  // One job as a row: identity, recent runs, last good copy, next run, Run/Stop.
  import type { Job, JobRun } from '$lib/types';
  import { jobStatus } from '$lib/utils/jobStatus';
  import { destinationKind, destinationTarget } from '$lib/utils/jobUtils';
  import { DESTINATION_ICON, destinationKindLabel } from '$lib/utils/destination';
  import { formatRelativeTime, formatTimeUntil, formatDateTime } from '$lib/i18n/relativeTime';
  import { isRunning, runJob, stopJob } from '$lib/jobActions';
  import { showRunLog } from '$lib/stores/runLog';
  import StatusChip from '../ui/StatusChip.svelte';
  import RunStrip from '../ui/RunStrip.svelte';
  import PathText from '../ui/PathText.svelte';
  import Icon from '../ui/Icon.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Menu from '../ui/Menu.svelte';
  import { jobMenuItems } from './jobMenu';
  import * as m from '$lib/paraglide/messages.js';

  let {
    job,
    runs = [],
    now = Date.now(),
    withMenu = false,
  }: { job: Job; runs?: JobRun[]; now?: number; withMenu?: boolean } = $props();

  const status = $derived(jobStatus(job, now));
  const kind = $derived(destinationKind(job));
  const running = $derived(isRunning(job));
  const next = $derived(
    job.enabled
      ? (job.schedules ?? [])
          .filter((s) => s.enabled && s.next_run_at)
          .map((s) => s.next_run_at as string)
          .sort((a, b) => Date.parse(a) - Date.parse(b))[0] ?? null
      : null,
  );
  let busy = $state(false);

  async function toggleRun(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    busy = true;
    if (running) await stopJob(job);
    else await runJob(job);
    busy = false;
  }
</script>

<div class="relative group grid grid-cols-[1fr_auto] lg:grid-cols-[minmax(0,1.4fr)_minmax(8rem,1fr)_8.5rem_8.5rem_auto] items-center gap-x-4 lg:gap-x-6 gap-y-2 px-4 sm:px-5 py-3.5 hover:bg-gray-50 dark:hover:bg-gray-800/60 transition-colors">
  <div class="min-w-0">
    <div class="flex items-center gap-2 min-w-0">
      <a href="#/jobs/{job.id}" class="font-medium text-gray-900 dark:text-white truncate after:absolute after:inset-0 focus-visible:outline-none">
        {job.name}
      </a>
      <StatusChip {status} size="sm" hasRun={!!job.last_run_status} />
    </div>
    <p class="mt-0.5 flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 min-w-0">
      <Icon name={DESTINATION_ICON[kind]} class="w-3.5 h-3.5 shrink-0" />
      <span class="shrink-0">{destinationKindLabel(kind)}</span>
      <span aria-hidden="true">·</span>
      <PathText path={destinationTarget(job)} max={32} class="truncate" />
    </p>
  </div>

  <div class="hidden lg:block relative z-10">
    {#if runs.length > 0}
      <RunStrip {runs} slots={16} height="h-6" onselect={(r) => showRunLog(r.id, job.id)} />
    {:else}
      <span class="text-xs text-gray-400 dark:text-gray-500">{m.overview_no_runs()}</span>
    {/if}
  </div>

  <div class="hidden lg:block text-sm">
    <p class="text-[11px] uppercase tracking-wider text-gray-400 dark:text-gray-500">{m.overview_last_good()}</p>
    <p class="text-gray-800 dark:text-gray-200 tabular-nums truncate" title={status.lastSuccessAt ? formatDateTime(status.lastSuccessAt) : ''}>
      {status.lastSuccessAt ? formatRelativeTime(status.lastSuccessAt) : m.common_never()}
    </p>
  </div>

  <div class="hidden lg:block text-sm">
    <p class="text-[11px] uppercase tracking-wider text-gray-400 dark:text-gray-500">{m.overview_next_run()}</p>
    <p class="text-gray-800 dark:text-gray-200 tabular-nums truncate" title={next ? formatDateTime(next) : ''}>
      {next ? formatTimeUntil(next) : m.overview_manual_only()}
    </p>
  </div>

  <div class="relative z-10 row-span-2 lg:row-span-1 self-start lg:self-center flex items-center gap-1">
    <button
      type="button"
      class="btn btn-icon btn-sm {running ? 'btn-danger-ghost' : 'btn-secondary'}"
      onclick={toggleRun}
      disabled={busy || (!job.enabled && !running)}
      aria-label={running ? m.job_card_tooltip_stop() : m.job_card_tooltip_start()}
      title={running ? m.job_card_tooltip_stop() : !job.enabled ? m.job_card_tooltip_enable() : m.job_card_tooltip_start()}
    >
      {#if busy}<Spinner />{:else}<Icon name={running ? 'stop' : 'play'} class="w-4 h-4" solid={!running} />{/if}
    </button>
    {#if withMenu}
      <Menu label={m.common_actions()} items={jobMenuItems(job)} triggerClass="btn btn-ghost btn-icon btn-sm" />
    {/if}
  </div>

  <!-- Phone: one line of facts plus the strip -->
  <div class="lg:hidden col-span-1 space-y-2">
    <p class="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
      {m.overview_last_good()}: {status.lastSuccessAt ? formatRelativeTime(status.lastSuccessAt) : m.common_never()}
      · {m.overview_next_run()}: {next ? formatTimeUntil(next) : m.overview_manual_only()}
    </p>
    {#if runs.length > 0}
      <div class="relative z-10"><RunStrip {runs} slots={20} height="h-5" barClass="w-1.5 shrink-0" onselect={(r) => showRunLog(r.id, job.id)} /></div>
    {/if}
  </div>
</div>

<script lang="ts">
  // A job as a card in the grid view.
  import type { Job, JobRun } from '$lib/types';
  import { jobStatus } from '$lib/utils/jobStatus';
  import { destinationKind, destinationTarget } from '$lib/utils/jobUtils';
  import { DESTINATION_ICON, destinationKindLabel } from '$lib/utils/destination';
  import { formatRelativeTime, formatTimeUntil, formatDateTime } from '$lib/i18n/relativeTime';
  import { formatBytes } from '$lib/utils/format';
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

  let { job, runs = [], now = Date.now() }: { job: Job; runs?: JobRun[]; now?: number } = $props();

  const status = $derived(jobStatus(job, now));
  const kind = $derived(destinationKind(job));
  const running = $derived(isRunning(job));
  const next = $derived(
    job.enabled
      ? ((job.schedules ?? [])
          .filter((s) => s.enabled && s.next_run_at)
          .map((s) => s.next_run_at as string)
          .sort((a, b) => Date.parse(a) - Date.parse(b))[0] ?? null)
      : null,
  );
  const used = $derived(
    job.dest_storage_total && job.dest_storage_free != null
      ? Math.round(((job.dest_storage_total - job.dest_storage_free) / job.dest_storage_total) * 100)
      : null,
  );
  let busy = $state(false);

  async function toggleRun() {
    busy = true;
    if (running) await stopJob(job);
    else await runJob(job);
    busy = false;
  }
</script>

<article class="card relative flex flex-col p-4 sm:p-5 hover:border-gray-300 dark:hover:border-gray-600 transition-colors {job.enabled ? '' : 'opacity-75'}">
  <div class="flex items-start justify-between gap-2">
    <div class="min-w-0">
      <h3 class="font-semibold text-gray-900 dark:text-white truncate">
        <a href="#/jobs/{job.id}" class="after:absolute after:inset-0 focus-visible:outline-none">{job.name}</a>
      </h3>
      {#if job.description}<p class="text-xs text-gray-500 dark:text-gray-400 truncate mt-0.5">{job.description}</p>{/if}
    </div>
    <div class="relative z-10 -mr-2 -mt-1">
      <Menu label={m.common_actions()} items={jobMenuItems(job)} triggerClass="btn btn-ghost btn-icon btn-sm" />
    </div>
  </div>

  <div class="mt-2"><StatusChip {status} hasRun={!!job.last_run_status} /></div>

  <dl class="mt-4 space-y-1.5 text-sm">
    <div class="flex items-center gap-2 min-w-0 text-gray-600 dark:text-gray-300">
      <dt class="shrink-0" title={m.job_sources()}><Icon name="folder" class="w-4 h-4 text-gray-400" /></dt>
      <dd class="min-w-0 truncate">
        {#if job.source_dirs.length === 1}
          <PathText path={job.source_dirs[0]} max={34} />
        {:else}
          {m.jobs_folders({ count: job.source_dirs.length })}
        {/if}
      </dd>
    </div>
    <div class="flex items-center gap-2 min-w-0 text-gray-600 dark:text-gray-300">
      <dt class="shrink-0 flex items-center gap-0.5 -ml-1" title={destinationKindLabel(kind)}>
        <Icon name="chevron-right" class="w-3 h-3 text-gray-400" strokeWidth={2.2} /><Icon name={DESTINATION_ICON[kind]} class="w-4 h-4 text-gray-400" />
      </dt>
      <dd class="min-w-0 truncate"><PathText path={destinationTarget(job)} max={34} /></dd>
    </div>
  </dl>

  {#if used != null}
    <div class="mt-3" title={m.storage_tooltip({ free: formatBytes(job.dest_storage_free ?? 0), total: formatBytes(job.dest_storage_total) })}>
      <div class="h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
        <div class="h-full rounded-full {used > 90 ? 'bg-red-500' : used > 75 ? 'bg-amber-500' : 'bg-gray-500 dark:bg-gray-400'}" style="width:{used}%"></div>
      </div>
      <p class="text-[11px] text-gray-500 dark:text-gray-400 mt-1 tabular-nums">{m.jobs_storage_free({ free: formatBytes(job.dest_storage_free ?? 0) })}</p>
    </div>
  {/if}

  <div class="mt-4 relative z-10">
    {#if runs.length > 0}
      <RunStrip {runs} slots={20} height="h-6" barClass="w-1.5 shrink-0" onselect={(r) => showRunLog(r.id, job.id)} />
    {:else}
      <div class="h-6 flex items-end"><span class="text-xs text-gray-400 dark:text-gray-500">{m.overview_no_runs()}</span></div>
    {/if}
  </div>

  <div class="mt-4 pt-3 border-t border-gray-200 dark:border-gray-700 flex items-end justify-between gap-3">
    <div class="grid grid-cols-2 gap-4 text-xs min-w-0">
      <div>
        <p class="text-gray-400 dark:text-gray-500">{m.overview_last_good()}</p>
        <p class="text-gray-800 dark:text-gray-200 tabular-nums truncate" title={status.lastSuccessAt ? formatDateTime(status.lastSuccessAt) : ''}>
          {status.lastSuccessAt ? formatRelativeTime(status.lastSuccessAt) : m.common_never()}
        </p>
      </div>
      <div>
        <p class="text-gray-400 dark:text-gray-500">{m.overview_next_run()}</p>
        <p class="text-gray-800 dark:text-gray-200 tabular-nums truncate" title={next ? formatDateTime(next) : ''}>
          {next ? formatTimeUntil(next) : m.overview_manual_only()}
        </p>
      </div>
    </div>
    <button
      type="button"
      class="relative z-10 btn btn-sm shrink-0 {running ? 'btn-danger-ghost' : 'btn-secondary'}"
      onclick={toggleRun}
      disabled={busy || (!job.enabled && !running)}
      title={!job.enabled && !running ? m.job_card_tooltip_enable() : undefined}
    >
      {#if busy}<Spinner />{:else}<Icon name={running ? 'stop' : 'play'} class="w-4 h-4" solid={!running} />{/if}
      {running ? m.job_btn_stop() : m.common_run()}
    </button>
  </div>
</article>

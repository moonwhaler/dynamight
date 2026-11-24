<script lang="ts">
  // Jobs as a table on wider screens (one header, dense rows), as rows on phones.
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
  import JobRow from './JobRow.svelte';
  import { jobMenuItems } from './jobMenu';
  import * as m from '$lib/paraglide/messages.js';

  type SortKey = 'name' | 'status' | 'last_success' | 'next_run';

  let {
    jobs,
    runs,
    now = Date.now(),
    sort,
    onsort,
  }: {
    jobs: Job[];
    runs: Map<number, JobRun[]>;
    now?: number;
    /** Current sort, to mark the header; omit for an unsortable table */
    sort?: { by: SortKey; order: 'asc' | 'desc' };
    onsort?: (by: SortKey) => void;
  } = $props();

  let busy = $state<number | null>(null);

  function nextRun(job: Job): string | null {
    if (!job.enabled) return null;
    return (
      (job.schedules ?? [])
        .filter((s) => s.enabled && s.next_run_at)
        .map((s) => s.next_run_at as string)
        .sort((a, b) => Date.parse(a) - Date.parse(b))[0] ?? null
    );
  }

  async function toggleRun(job: Job) {
    busy = job.id;
    if (isRunning(job)) await stopJob(job);
    else await runJob(job);
    busy = null;
  }
</script>

{#snippet header(key: SortKey, label: string, cls = '')}
  <th class={cls} aria-sort={sort?.by === key ? (sort.order === 'asc' ? 'ascending' : 'descending') : undefined}>
    {#if onsort}
      <button type="button" class="inline-flex items-center gap-1 hover:text-gray-900 dark:hover:text-white" onclick={() => onsort(key)}>
        {label}
        {#if sort?.by === key}
          <Icon name="chevron-down" class="w-3.5 h-3.5 {sort.order === 'asc' ? 'rotate-180' : ''}" strokeWidth={2} />
        {/if}
      </button>
    {:else}
      {label}
    {/if}
  </th>
{/snippet}

<!-- Phones -->
<div class="md:hidden card rows overflow-hidden">
  {#each jobs as job (job.id)}
    <JobRow {job} runs={runs.get(job.id) ?? []} {now} withMenu />
  {/each}
</div>

<!-- Tablets and desktop -->
<div class="hidden md:block card overflow-hidden">
  <table class="table">
    <thead>
      <tr>
        {@render header('name', m.history_table_job())}
        {@render header('status', m.jobs_sort_status())}
        {@render header('last_success', m.overview_last_good())}
        {@render header('next_run', m.overview_next_run())}
        <th class="hidden lg:table-cell">{m.jobs_col_recent()}</th>
        <th class="w-px"><span class="sr-only">{m.common_actions()}</span></th>
      </tr>
    </thead>
    <tbody>
      {#each jobs as job (job.id)}
        {@const status = jobStatus(job, now)}
        {@const running = isRunning(job)}
        {@const next = nextRun(job)}
        {@const kind = destinationKind(job)}
        <tr class="relative">
          <td class="max-w-0 w-[38%]">
            <a href="#/jobs/{job.id}" class="font-medium text-gray-900 dark:text-white hover:underline block truncate">{job.name}</a>
            <span class="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 min-w-0 mt-0.5">
              <Icon name={DESTINATION_ICON[kind]} class="w-3.5 h-3.5 shrink-0" />
              <span class="shrink-0">{destinationKindLabel(kind)}</span>
              <span aria-hidden="true">·</span>
              <PathText path={destinationTarget(job)} max={30} class="truncate" />
            </span>
          </td>
          <td class="whitespace-nowrap"><StatusChip {status} hasRun={!!job.last_run_status} /></td>
          <td class="whitespace-nowrap tabular-nums text-gray-700 dark:text-gray-300" title={status.lastSuccessAt ? formatDateTime(status.lastSuccessAt) : ''}>
            {status.lastSuccessAt ? formatRelativeTime(status.lastSuccessAt) : m.common_never()}
          </td>
          <td class="whitespace-nowrap tabular-nums text-gray-700 dark:text-gray-300" title={next ? formatDateTime(next) : ''}>
            {next ? formatTimeUntil(next) : m.overview_manual_only()}
          </td>
          <td class="hidden lg:table-cell">
            {#if (runs.get(job.id) ?? []).length > 0}
              <RunStrip runs={runs.get(job.id) ?? []} slots={12} height="h-5" barClass="w-1.5 shrink-0" onselect={(r) => showRunLog(r.id, job.id)} />
            {:else}
              <span class="text-xs text-gray-400 dark:text-gray-500">—</span>
            {/if}
          </td>
          <td class="whitespace-nowrap">
            <div class="flex items-center justify-end gap-1">
              <button
                type="button"
                class="btn btn-icon btn-sm {running ? 'btn-danger-ghost' : 'btn-ghost'}"
                onclick={() => toggleRun(job)}
                disabled={busy === job.id || (!job.enabled && !running)}
                aria-label={running ? m.job_card_tooltip_stop() : m.job_card_tooltip_start()}
                title={running ? m.job_card_tooltip_stop() : !job.enabled ? m.job_card_tooltip_enable() : m.job_card_tooltip_start()}
              >
                {#if busy === job.id}<Spinner />{:else}<Icon name={running ? 'stop' : 'play'} class="w-4 h-4" solid={!running} />{/if}
              </button>
              <Menu label={m.common_actions()} items={jobMenuItems(job)} triggerClass="btn btn-ghost btn-icon btn-sm" />
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

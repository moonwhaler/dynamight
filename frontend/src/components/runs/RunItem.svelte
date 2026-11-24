<script lang="ts">
  // One run in a list: outcome, job, when, how long, how much. Opens its log.
  import type { JobRun } from '$lib/types';
  import { runDurationMs, runHasResult } from '$lib/utils/jobStatus';
  import { formatStatus } from '$lib/i18n/status';
  import { formatRelativeTime, formatDateTime, formatDuration } from '$lib/i18n/relativeTime';
  import { formatBytes } from '$lib/utils/format';
  import { showRunLog } from '$lib/stores/runLog';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { run, jobName, showJob = true }: { run: JobRun; jobName?: string; showJob?: boolean } = $props();

  const duration = $derived(runDurationMs(run));
  const hasResult = $derived(runHasResult(run));
  const note = $derived(typeof run.summary === 'string' && run.summary ? run.summary : null);

  const tone: Record<JobRun['status'], string> = {
    completed: 'text-green-600 dark:text-green-400',
    failed: 'text-red-600 dark:text-red-400',
    cancelled: 'text-amber-600 dark:text-amber-400',
    running: 'text-primary-600 dark:text-primary-400',
    pending: 'text-primary-600 dark:text-primary-400',
  };
  const icon = {
    completed: 'check-circle',
    failed: 'x-circle',
    cancelled: 'pause',
    running: 'refresh',
    pending: 'clock',
  } as const;
</script>

<button
  type="button"
  onclick={() => showRunLog(run.id, run.job_id)}
  class="w-full text-left flex items-start gap-3 px-4 sm:px-5 py-3 hover:bg-gray-50 dark:hover:bg-gray-800/60 transition-colors"
>
  <span class="mt-0.5 shrink-0 {tone[run.status]}" title={formatStatus(run.status)}>
    <Icon name={icon[run.status]} class="w-5 h-5 {run.status === 'running' ? 'animate-spin [animation-duration:2.5s]' : ''}" />
  </span>
  <span class="flex-1 min-w-0">
    <span class="flex items-baseline justify-between gap-3">
      <span class="font-medium text-gray-900 dark:text-white truncate">
        {#if showJob}{jobName ?? `#${run.job_id}`}{:else}{m.run_number({ id: run.id })}{/if}
        <span class="sr-only">{formatStatus(run.status)}</span>
      </span>
      <span class="shrink-0 text-xs text-gray-500 dark:text-gray-400 tabular-nums" title={run.started_at ? formatDateTime(run.started_at) : ''}>
        {formatRelativeTime(run.started_at)}
      </span>
    </span>
    <span class="block text-xs text-gray-500 dark:text-gray-400 tabular-nums mt-0.5">
      {formatStatus(run.status)}
      {#if duration != null} · {formatDuration(duration)}{/if}
      {#if hasResult && run.bytes_transferred}
        · {formatBytes(run.bytes_transferred)}
      {/if}
      {#if hasResult && run.files_transferred}
        · {m.run_files({ count: run.files_transferred })}
      {/if}
    </span>
    {#if note && run.status !== 'completed'}
      <span class="block text-xs mt-1 font-mono truncate {run.status === 'failed' ? 'text-red-600 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'}" title={note}>
        {note}
      </span>
    {/if}
  </span>
</button>

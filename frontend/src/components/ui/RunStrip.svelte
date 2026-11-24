<script lang="ts">
  // The run history of one job as a row of bars, oldest left, newest right.
  // Color shows the outcome, height the duration relative to the others.
  import type { JobRun } from '$lib/types';
  import { runDurationMs } from '$lib/utils/jobStatus';
  import { formatStatus } from '$lib/i18n/status';
  import { formatDateTime, formatDuration } from '$lib/i18n/relativeTime';
  import * as m from '$lib/paraglide/messages.js';

  let {
    runs,
    slots = 20,
    height = 'h-7',
    barClass = 'w-2 shrink-0',
    onselect,
  }: {
    /** Newest first, as the API returns them */
    runs: JobRun[];
    slots?: number;
    height?: string;
    /** Width of one bar; runs only, no placeholders for missing ones */
    barClass?: string;
    onselect?: (run: JobRun) => void;
  } = $props();

  const shown = $derived(runs.slice(0, slots).reverse());
  const longest = $derived(Math.max(1, ...shown.map((r) => runDurationMs(r) ?? 0)));

  function color(status: JobRun['status']): string {
    switch (status) {
      case 'completed':
        return 'bg-green-500/80 dark:bg-green-500/70';
      case 'failed':
        return 'bg-red-500';
      case 'cancelled':
        return 'bg-amber-400 dark:bg-amber-500/80';
      case 'running':
      case 'pending':
        return 'bg-primary-500 animate-pulse';
    }
  }

  function barHeight(run: JobRun): string {
    const d = runDurationMs(run) ?? 0;
    // Short runs stay visible; failures are always full height
    const pct = run.status === 'failed' ? 100 : 30 + Math.round((d / longest) * 70);
    return `${pct}%`;
  }

  function describe(run: JobRun): string {
    const d = runDurationMs(run);
    return [
      m.run_number({ id: run.id }),
      formatStatus(run.status),
      run.started_at ? formatDateTime(run.started_at) : '',
      d != null ? formatDuration(d) : '',
    ]
      .filter(Boolean)
      .join(' · ');
  }
</script>

<div class="flex items-end gap-[3px] {height}" role="group" aria-label={m.run_strip_label()}>
  {#each shown as run (run.id)}
    {#if onselect}
      <button
        type="button"
        class="{barClass} rounded-[2px] {color(run.status)} hover:opacity-70 focus-visible:outline-offset-1 transition-opacity"
        style="height:{barHeight(run)}"
        title={describe(run)}
        aria-label={describe(run)}
        onclick={(e) => {
          e.stopPropagation();
          onselect(run);
        }}
      ></button>
    {:else}
      <span
        role="img"
        class="{barClass} rounded-[2px] {color(run.status)}"
        style="height:{barHeight(run)}"
        title={describe(run)}
        aria-label={describe(run)}
      ></span>
    {/if}
  {/each}
</div>

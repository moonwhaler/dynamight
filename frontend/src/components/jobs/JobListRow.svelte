<script lang="ts">
  import type { Job, Schedule } from '../../lib/types';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import { tablePreferencesStore } from '../../lib/stores/tablePreferences';
  import { get } from 'svelte/store';
  import Spinner from '../ui/Spinner.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import { formatRelativeTime, formatTimeUntil } from '$lib/i18n/relativeTime';
  import { getStatusIndicator } from '$lib/i18n/status';
  import { getDestinationLabel } from '$lib/utils/jobUtils';
  import * as m from '$lib/paraglide/messages.js';

  let { job, onShowLogs }: { job: Job; onShowLogs: (runId: number) => void } = $props();
  let starting = $state(false);
  let stopping = $state(false);
  let toggling = $state(false);
  let loadingLogs = $state(false);

  const isRunning = $derived(job.last_run_status === 'running' || job.last_run_status === 'pending');
  const statusInfo = $derived(getStatusIndicator(job.last_run_status));
  const timeAgo = $derived(formatRelativeTime(job.last_run_at));

  const DAY_NAMES = [
    () => m.day_sunday(),
    () => m.day_monday(),
    () => m.day_tuesday(),
    () => m.day_wednesday(),
    () => m.day_thursday(),
    () => m.day_friday(),
    () => m.day_saturday(),
  ];

  function dayName(dow: number): string {
    return DAY_NAMES[dow]?.() ?? '?';
  }

  function toTime(h: string, min: string): string {
    const hh = parseInt(h, 10);
    const mm = parseInt(min, 10);
    if (!isFinite(hh) || !isFinite(mm)) return `${h}:${min}`;
    return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}`;
  }

  function parseCronToHuman(expr: string): string {
    const parts = expr.trim().split(/\s+/);
    if (parts.length !== 5) return m.jobs_sched_custom({ cron: expr });
    const [min, hour, dom, month, dow] = parts;

    if (min.startsWith('*/') && hour === '*' && dom === '*' && month === '*' && dow === '*') {
      return m.jobs_sched_interval_minutes({ count: min.slice(2) });
    }
    if ((min === '0' || min === '*') && hour.startsWith('*/') && dom === '*' && month === '*' && dow === '*') {
      return m.jobs_sched_interval_hours({ count: hour.slice(2) });
    }

    const hNum = parseInt(hour, 10);
    const mNum = parseInt(min, 10);
    const hasExactTime = isFinite(hNum) && isFinite(mNum);
    if (!hasExactTime) return m.jobs_sched_custom({ cron: expr });
    const time = toTime(hour, min);

    if (dom === '*' && month === '*' && dow === '1-5') return m.jobs_sched_weekdays({ time });
    if (dom === '*' && month === '*' && (dow === '0,6' || dow === '6,0')) return m.jobs_sched_weekends({ time });

    const dowNum = parseInt(dow, 10);
    if (dom === '*' && month === '*' && isFinite(dowNum) && dowNum >= 0 && dowNum <= 6 && dow === String(dowNum)) {
      return m.jobs_sched_weekly({ day: dayName(dowNum), time });
    }

    const domNum = parseInt(dom, 10);
    if (dow === '*' && month === '*' && isFinite(domNum) && dom === String(domNum)) {
      return m.jobs_sched_monthly({ day: domNum, time });
    }
    if (dom === '*' && month === '*' && dow === '*') return m.jobs_sched_daily({ time });

    return m.jobs_sched_custom({ cron: expr });
  }

  function formatSchedule(sched: Schedule): string {
    const type = sched.schedule_type;
    const time = sched.time_of_day ?? '';

    if (type === 'daily') return m.jobs_sched_daily({ time });
    if (type === 'weekly') {
      const day = sched.day_of_week != null ? dayName(sched.day_of_week) : '?';
      return m.jobs_sched_weekly({ day, time });
    }
    if (type === 'monthly') return m.jobs_sched_monthly({ day: sched.day_of_month ?? 1, time });
    return parseCronToHuman(sched.cron_expression);
  }

  async function handleRun() {
    if (starting || isRunning) return;
    starting = true;
    try {
      const result = await api.jobs.run(job.id);
      if ($preferencesStore.showLogViewerAfterManualRun) {
        onShowLogs(result.runId);
      }
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.job_error_start(), variant: 'error' });
    } finally {
      starting = false;
    }
  }

  async function handleStop() {
    if (stopping || !isRunning) return;

    if (get(preferencesStore).confirmKillProcess) {
      const confirmed = await confirm({
        title: m.kill_confirm_title(),
        message: m.kill_confirm_message(),
        confirmText: m.kill_confirm_button(),
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    stopping = true;
    try {
      await api.jobs.cancel(job.id);
      await jobsStore.refresh();
    } catch {
      // Ignore errors
    } finally {
      stopping = false;
    }
  }

  async function handleViewLogs() {
    if (loadingLogs || !isRunning) return;
    loadingLogs = true;
    try {
      const runs = await api.runs.list(job.id, 1, 0);
      if (runs.length > 0 && (runs[0].status === 'running' || runs[0].status === 'pending')) {
        onShowLogs(runs[0].id);
      }
    } catch {
      // Ignore errors
    } finally {
      loadingLogs = false;
    }
  }

  async function handleToggleEnabled(event: Event) {
    event.preventDefault();
    event.stopPropagation();
    if (toggling) return;
    toggling = true;
    try {
      const updatedJob = await api.jobs.update(job.id, { enabled: !job.enabled });
      jobsStore.updateJob(updatedJob);
    } catch {
      // Ignore
    } finally {
      toggling = false;
    }
  }
</script>

<tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50">
  {#each $tablePreferencesStore.visibleColumns as col (col)}
    {#if col === 'job'}
      <td class="px-4 py-3">
        <a href="#/jobs/{job.id}" class="flex items-center gap-2 group">
          {#if statusInfo.color}
            <span
              class="inline-block w-2.5 h-2.5 rounded-full {statusInfo.color} shrink-0"
              title="{statusInfo.label}{timeAgo ? ` (${timeAgo})` : ''}"
            ></span>
          {:else}
            <span class="inline-block w-2.5 h-2.5 rounded-full bg-gray-300 dark:bg-gray-600 shrink-0"></span>
          {/if}
          <span class="text-sm font-medium text-gray-900 dark:text-gray-100 group-hover:text-primary-600 dark:group-hover:text-primary-400 truncate">
            {job.name}
          </span>
        </a>
      </td>
    {:else if col === 'status'}
      <td class="px-4 py-3 whitespace-nowrap">
        <div class="w-[4.5rem]">
          <button
            onclick={handleToggleEnabled}
            disabled={toggling || isRunning}
            class="badge {isRunning ? 'badge-info' : job.enabled ? 'badge-success' : 'badge-gray'} {isRunning ? '' : 'cursor-pointer hover:opacity-80'} transition-opacity"
            title={isRunning ? m.job_card_currently_running() : job.enabled ? m.job_card_click_disable() : m.job_card_click_enable()}
          >
            {toggling ? '...' : isRunning ? m.common_running() : job.enabled ? m.common_active() : m.common_disabled()}
          </button>
        </div>
      </td>
    {:else if col === 'sources'}
      <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
        {m.jobs_sources_count({ count: job.source_dirs.length })}
      </td>
    {:else if col === 'destination'}
      {@const destLabel = getDestinationLabel(job)}
      <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 truncate" title={destLabel}>
        {destLabel}
      </td>
    {:else if col === 'last_run'}
      <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">
        {timeAgo}
      </td>
    {:else if col === 'schedule'}
      <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
        {#if !job.schedules?.length}
          <span class="text-gray-300 dark:text-gray-600">—</span>
        {:else}
          {@const sched = job.schedules[0]}
          <div class="flex flex-col gap-0.5 min-w-0">
            <span class="truncate {!sched.enabled ? 'line-through opacity-50' : ''}">
              {formatSchedule(sched)}
            </span>
            {#if sched.next_run_at && sched.enabled}
              <span class="text-xs text-gray-400 dark:text-gray-500">
                {m.jobs_schedule_next({ time: formatTimeUntil(sched.next_run_at) })}
              </span>
            {/if}
            {#if job.schedules.length > 1}
              <span class="text-xs text-gray-400">+{job.schedules.length - 1} {m.jobs_schedule_more()}</span>
            {/if}
          </div>
        {/if}
      </td>
    {:else if col === 'options'}
      <td class="px-4 py-3">
        <div class="flex gap-1 flex-wrap">
          {#if job.sync_deletes}
            <span class="badge badge-warning text-xs">{m.job_card_badge_mirror()}</span>
          {/if}
          {#if job.compress}
            <span class="badge badge-info text-xs">{m.job_card_badge_compress()}</span>
          {/if}
          {#if job.dry_run}
            <span class="badge badge-gray text-xs">{m.job_card_badge_dry_run()}</span>
          {/if}
        </div>
      </td>
    {:else if col === 'actions'}
      <td class="px-4 py-3">
        {#if isRunning || stopping}
          <div class="flex items-center gap-1.5">
            <button
              onclick={handleViewLogs}
              disabled={loadingLogs}
              class="btn btn-sm w-9 h-9 p-0 flex items-center justify-center bg-blue-50 text-blue-600 hover:bg-blue-100 dark:bg-blue-900/30 dark:text-blue-400 dark:hover:bg-blue-900/50 border border-blue-200 dark:border-blue-800"
              title={m.job_card_tooltip_view_logs()}
            >
              {#if loadingLogs}
                <Spinner />
              {:else}
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 7.5l3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0021 18V6a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 003 6v12a2.25 2.25 0 002.25 2.25z" />
                </svg>
              {/if}
            </button>
            <button
              onclick={handleStop}
              disabled={stopping}
              class="btn btn-sm btn-danger w-9 h-9 p-0 flex items-center justify-center"
              title={m.job_card_tooltip_stop()}
            >
              {#if stopping}
                <Spinner />
              {:else}
                <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                  <rect x="6" y="6" width="12" height="12" rx="1" />
                </svg>
              {/if}
            </button>
          </div>
        {:else}
          <button
            onclick={handleRun}
            disabled={starting || !job.enabled}
            class="btn btn-sm btn-secondary w-9 h-9 p-0 flex items-center justify-center"
            title={!job.enabled ? m.job_card_tooltip_enable() : m.job_card_tooltip_start()}
          >
            {#if starting}
              <Spinner />
            {:else}
              <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
              </svg>
            {/if}
          </button>
        {/if}
      </td>
    {/if}
  {/each}
</tr>

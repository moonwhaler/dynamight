<script lang="ts">
  import { api } from '../../lib/api';
  import type { Schedule } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import { describeCron } from '$lib/utils/cron';
  import { serverTimezone, differingTimezone, formatScheduleTime } from '$lib/stores/serverTimezone';
  import * as m from '$lib/paraglide/messages.js';

  // Schedule changes are a draft: they are applied when the job is saved
  // (see JobDetail), so cancelling the form also discards them.
  let {
    schedules = $bindable<Schedule[]>([]),
    pendingSchedules = $bindable<string[]>([]),
    dirty = false,
  }: {
    /** Saved schedules; removing or toggling one only changes this list */
    schedules: Schedule[];
    /** Cron expressions to create on save */
    pendingSchedules?: string[];
    /** Whether the draft differs from what is saved */
    dirty?: boolean;
  } = $props();

  let showAdd = $state(false);
  let saving = $state(false);
  let scheduleType = $state<'daily' | 'weekly' | 'monthly' | 'interval' | 'custom'>('daily');
  let timeOfDay = $state('02:00');
  let selectedDays = $state<number[]>([1]); // Default to Monday
  let dayOfMonth = $state(1);
  let cronExpression = $state('0 2 * * *');

  // Interval settings
  let intervalValue = $state(1);
  let intervalUnit = $state<'minutes' | 'hours'>('hours');

  function getDayName(index: number): string {
    const names = [m.day_sun(), m.day_mon(), m.day_tue(), m.day_wed(), m.day_thu(), m.day_fri(), m.day_sat()];
    return names[index];
  }

  function getDayNameFull(index: number): string {
    const names = [m.day_sunday(), m.day_monday(), m.day_tuesday(), m.day_wednesday(), m.day_thursday(), m.day_friday(), m.day_saturday()];
    return names[index];
  }

  function toggleDay(day: number) {
    if (selectedDays.includes(day)) {
      if (selectedDays.length > 1) {
        selectedDays = selectedDays.filter(d => d !== day);
      }
    } else {
      selectedDays = [...selectedDays, day].sort((a, b) => a - b);
    }
  }

  function buildCronExpression(): string {
    const [hours, minutes] = timeOfDay.split(':').map(Number);

    switch (scheduleType) {
      case 'daily':
        return `${minutes} ${hours} * * *`;
      case 'weekly':
        return `${minutes} ${hours} * * ${selectedDays.join(',')}`;
      case 'monthly':
        return `${minutes} ${hours} ${dayOfMonth} * *`;
      case 'interval':
        if (intervalUnit === 'minutes') {
          return `*/${intervalValue} * * * *`;
        } else {
          return `0 */${intervalValue} * * *`;
        }
      case 'custom':
        return cronExpression;
      default:
        return '0 2 * * *';
    }
  }

  async function addSchedule() {
    saving = true;
    try {
      const cron = buildCronExpression();
      // The other types are built from valid parts; a typed expression is
      // checked by the server's parser before it joins the draft
      if (scheduleType === 'custom') {
        await api.schedules.validate(cron);
      }
      pendingSchedules = [...pendingSchedules, cron];
      showAdd = false;
      resetForm();
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      saving = false;
    }
  }

  function removePendingSchedule(index: number) {
    pendingSchedules = pendingSchedules.filter((_, i) => i !== index);
  }

  function deleteSchedule(id: number) {
    schedules = schedules.filter((s) => s.id !== id);
  }

  function toggleSchedule(schedule: Schedule) {
    schedules = schedules.map((s) => (s.id === schedule.id ? { ...s, enabled: !s.enabled } : s));
  }

  function resetForm() {
    scheduleType = 'daily';
    timeOfDay = '02:00';
    selectedDays = [1];
    dayOfMonth = 1;
    cronExpression = '0 2 * * *';
    intervalValue = 1;
    intervalUnit = 'hours';
  }

  function formatSchedule(schedule: Schedule): string {
    return formatCron(schedule.cron_expression);
  }

  function formatCron(cron: string): string {
    const d = describeCron(cron);
    switch (d.kind) {
      case 'every_minutes':
        return m.schedule_every_minutes({ count: d.count });
      case 'every_hours':
        return m.schedule_every_hours({ count: d.count });
      case 'daily':
        return m.schedule_daily_at({ time: d.time });
      case 'days':
        return m.schedule_days_at({ days: d.days.map(getDayName).join(', '), time: d.time });
      case 'monthly':
        return m.schedule_monthly_on({ day: d.day, time: d.time });
      default:
        return cron;
    }
  }

  function formatNextRun(date: string | null): string {
    if (!date) return m.schedule_not_scheduled();
    // In the server's zone when it differs, so it matches "Daily at 02:00"
    return formatScheduleTime(date, $serverTimezone);
  }

  const otherZone = $derived(differingTimezone($serverTimezone));

  // Preview the cron expression
  let cronPreview = $derived(buildCronExpression());
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
      {m.schedule_title()}
      <HelpTooltip text={m.schedule_help()} />
    </h2>
    {#if !showAdd}
      <button type="button" onclick={() => (showAdd = true)} class="btn btn-secondary text-sm">
        {m.schedule_add()}
      </button>
    {/if}
  </div>

  {#if schedules.length === 0 && pendingSchedules.length === 0 && !showAdd}
    <p class="text-gray-500 dark:text-gray-400 text-sm">{m.schedule_no_schedules()}</p>
  {/if}

  {#if otherZone}
    <p class="text-sm text-gray-500 dark:text-gray-400">{m.schedule_timezone_hint({ tz: otherZone })}</p>
  {/if}

  {#if dirty}
    <p class="text-sm text-amber-600 dark:text-amber-400">{m.schedule_changes_unsaved()}</p>
  {/if}

  <!-- Pending Schedules (for new jobs) -->
  {#each pendingSchedules as cron, index (index)}
    <div class="flex items-center justify-between p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
      <div class="flex items-center gap-3">
        <div class="w-5 h-5 flex items-center justify-center">
          <svg class="w-4 h-4 text-amber-600 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div>
          <div class="font-medium text-gray-900 dark:text-white">{formatCron(cron)}</div>
          <div class="text-sm text-amber-600 dark:text-amber-400">{m.schedule_pending()}</div>
        </div>
      </div>
      <button
        type="button"
        onclick={() => removePendingSchedule(index)}
        class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
        aria-label={m.common_delete()}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
      </button>
    </div>
  {/each}

  <!-- Existing Schedules -->
  {#each schedules as schedule (schedule.id)}
    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-700 rounded-lg">
      <div class="flex items-center gap-3">
        <input
          type="checkbox"
          checked={schedule.enabled}
          onchange={() => toggleSchedule(schedule)}
          class="rounded text-primary-600"
        />
        <div>
          <div class="font-medium text-gray-900 dark:text-white">{formatSchedule(schedule)}</div>
          <div class="text-sm text-gray-500 dark:text-gray-400">{m.schedule_next({ time: formatNextRun(schedule.next_run_at) })}</div>
        </div>
      </div>
      <button
        type="button"
        onclick={() => deleteSchedule(schedule.id)}
        class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
        aria-label={m.common_delete()}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
      </button>
    </div>
  {/each}

  <!-- Add Schedule Form -->
  {#if showAdd}
    <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-4">
      <div>
        <label for="scheduleType" class="label">{m.schedule_type()}</label>
        <select id="scheduleType" bind:value={scheduleType} class="input">
          <option value="daily">{m.schedule_type_daily()}</option>
          <option value="weekly">{m.schedule_type_weekly()}</option>
          <option value="monthly">{m.schedule_type_monthly()}</option>
          <option value="interval">{m.schedule_type_interval()}</option>
          <option value="custom">{m.schedule_type_custom()}</option>
        </select>
      </div>

      {#if scheduleType === 'daily'}
        <div>
          <label for="dailyTime" class="label">{m.schedule_time()}</label>
          <input id="dailyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
      {/if}

      {#if scheduleType === 'weekly'}
        <div>
          <label for="weeklyTime" class="label">{m.schedule_time()}</label>
          <input id="weeklyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
        <div>
          <label class="label">
            {m.schedule_days_of_week()}
            <HelpTooltip text={m.schedule_days_help()} />
          </label>
          <div class="flex flex-wrap gap-2 mt-2">
            {#each [0, 1, 2, 3, 4, 5, 6] as index}
              <button
                type="button"
                onclick={() => toggleDay(index)}
                class="px-3 py-1.5 text-sm rounded-lg border transition-colors {selectedDays.includes(index)
                  ? 'bg-primary-600 text-white border-primary-600'
                  : 'bg-white dark:bg-gray-700 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-primary-400'}"
              >
                {getDayName(index)}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if scheduleType === 'monthly'}
        <div>
          <label for="monthlyTime" class="label">{m.schedule_time()}</label>
          <input id="monthlyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
        <div>
          <label for="dayOfMonth" class="label">{m.schedule_day_of_month()}</label>
          <input
            id="dayOfMonth"
            type="number"
            bind:value={dayOfMonth}
            min="1"
            max="31"
            class="input w-20"
          />
        </div>
      {/if}

      {#if scheduleType === 'interval'}
        <div class="flex items-end gap-3">
          <div>
            <label class="label">
              {m.schedule_run_every()}
              <HelpTooltip text={m.schedule_interval_help()} />
            </label>
            <input
              type="number"
              bind:value={intervalValue}
              min="1"
              max={intervalUnit === 'minutes' ? 59 : 23}
              class="input w-20"
            />
          </div>
          <div>
            <select bind:value={intervalUnit} class="input">
              <option value="minutes">{m.schedule_minutes()}</option>
              <option value="hours">{m.schedule_hours()}</option>
            </select>
          </div>
        </div>
      {/if}

      {#if scheduleType === 'custom'}
        <div>
          <label class="label">
            {m.schedule_cron()}
            <HelpTooltip text={m.schedule_cron_help()} />
          </label>
          <input
            type="text"
            bind:value={cronExpression}
            placeholder={m.schedule_cron_placeholder()}
            class="input font-mono"
          />
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
            {m.schedule_cron_format()}
          </p>
        </div>
      {/if}

      <!-- Cron Preview -->
      {#if scheduleType !== 'custom'}
        <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-3">
          <div class="text-xs text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-1">{m.schedule_cron()}</div>
          <code class="text-sm text-gray-800 dark:text-gray-200 font-mono">{cronPreview}</code>
        </div>
      {/if}

      <div class="flex gap-2">
        <button type="button" onclick={addSchedule} disabled={saving} class="btn btn-primary">
          {saving ? m.schedule_adding() : m.schedule_add()}
        </button>
        <button type="button"
          onclick={() => {
            showAdd = false;
            resetForm();
          }}
          class="btn btn-secondary"
        >
          {m.common_cancel()}
        </button>
      </div>
    </div>
  {/if}
</div>

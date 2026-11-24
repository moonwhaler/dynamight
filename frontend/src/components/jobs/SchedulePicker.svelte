<script lang="ts">
  import { api } from '../../lib/api';
  import type { Schedule } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import { describeCron } from '$lib/utils/cron';
  import { serverTimezone, differingTimezone, formatScheduleTime } from '$lib/stores/serverTimezone';
  import * as m from '$lib/paraglide/messages.js';
  import { formatCron } from '$lib/i18n/schedule';
  import Toggle from '../ui/Toggle.svelte';
  import Icon from '../ui/Icon.svelte';

  // Schedule changes are a draft: they are applied when the job is saved
  // (see JobDraft), so discarding the form also discards them.
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
      const cron = buildCronExpression().trim().replace(/\s+/g, ' ');
      const existing = [...schedules.map((s) => s.cron_expression), ...pendingSchedules].map((c) => c.trim().replace(/\s+/g, ' '));
      if (existing.includes(cron)) {
        showToast({ message: m.schedule_duplicate({ when: formatCron(cron) }), variant: 'error' });
        return;
      }
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
  {#if otherZone}
    <p class="text-sm text-gray-500 dark:text-gray-400">{m.schedule_timezone_hint({ tz: otherZone })}</p>
  {/if}

  {#if schedules.length === 0 && pendingSchedules.length === 0 && !showAdd}
    <p class="text-sm text-gray-500 dark:text-gray-400">{m.schedule_no_schedules()}</p>
  {/if}

  {#if schedules.length > 0 || pendingSchedules.length > 0}
    <ul class="rows rounded-lg border border-gray-200 dark:border-gray-700">
      {#each schedules as schedule (schedule.id)}
        <li class="flex items-center gap-3 px-4 py-3">
          <div class="flex-1 min-w-0">
            <Toggle
              size="sm"
              label={formatSchedule(schedule)}
              description={schedule.enabled ? m.schedule_next({ time: formatNextRun(schedule.next_run_at) }) : m.jobview_paused()}
              checked={schedule.enabled}
              onchange={() => toggleSchedule(schedule)}
            />
          </div>
          <button type="button" onclick={() => deleteSchedule(schedule.id)} class="btn btn-ghost btn-icon btn-sm shrink-0" aria-label={m.schedule_remove()} title={m.schedule_remove()}>
            <Icon name="trash" class="w-4 h-4" />
          </button>
        </li>
      {/each}
      {#each pendingSchedules as cron, index (index)}
        <li class="flex items-center gap-3 px-4 py-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-gray-100">{formatCron(cron)}</p>
            <p class="text-xs text-primary-700 dark:text-primary-400 mt-0.5">{m.schedule_pending()}</p>
          </div>
          <button type="button" onclick={() => removePendingSchedule(index)} class="btn btn-ghost btn-icon btn-sm shrink-0" aria-label={m.schedule_remove()} title={m.schedule_remove()}>
            <Icon name="trash" class="w-4 h-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if dirty}
    <p class="text-sm text-amber-700 dark:text-amber-400">{m.schedule_changes_unsaved()}</p>
  {/if}

  {#if !showAdd}
    <button type="button" onclick={() => (showAdd = true)} class="btn btn-secondary">
      <Icon name="plus" class="w-4 h-4" />{m.schedule_add()}
    </button>
  {/if}

  <!-- Add Schedule Form -->
  {#if showAdd}
    <div class="rounded-lg bg-gray-100/70 dark:bg-gray-900/50 p-4 space-y-4">
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
        <div>
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

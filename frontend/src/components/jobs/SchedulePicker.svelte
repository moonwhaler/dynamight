<script lang="ts">
  import { api } from '../../lib/api';
  import type { Schedule, CreateScheduleRequest } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';

  let {
    jobId,
    schedules = $bindable<Schedule[]>([]),
  }: {
    jobId: number;
    schedules: Schedule[];
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

  const dayNames = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const dayNamesFull = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

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
      const request: CreateScheduleRequest = { cron_expression: cron };

      const schedule = await api.schedules.create(jobId, request);
      schedules = [...schedules, schedule];
      showAdd = false;
      resetForm();
    } catch {
      // Ignore
    } finally {
      saving = false;
    }
  }

  async function deleteSchedule(id: number) {
    try {
      await api.schedules.delete(id);
      schedules = schedules.filter((s) => s.id !== id);
    } catch {
      // Ignore
    }
  }

  async function toggleSchedule(schedule: Schedule) {
    try {
      const updated = await api.schedules.update(schedule.id, { enabled: !schedule.enabled });
      schedules = schedules.map((s) => (s.id === updated.id ? updated : s));
    } catch {
      // Ignore
    }
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
    // Try to parse the cron expression for a human-readable format
    const cron = schedule.cron_expression;
    const parts = cron.split(' ');

    if (parts.length !== 5) return cron;

    const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

    // Interval: */N minutes
    if (minute.startsWith('*/') && hour === '*') {
      const mins = minute.slice(2);
      return `Every ${mins} minute${mins === '1' ? '' : 's'}`;
    }

    // Interval: */N hours
    if (minute === '0' && hour.startsWith('*/')) {
      const hrs = hour.slice(2);
      return `Every ${hrs} hour${hrs === '1' ? '' : 's'}`;
    }

    const time = `${hour.padStart(2, '0')}:${minute.padStart(2, '0')}`;

    // Daily
    if (dayOfMonth === '*' && month === '*' && dayOfWeek === '*') {
      return `Daily at ${time}`;
    }

    // Weekly (specific days)
    if (dayOfMonth === '*' && month === '*' && dayOfWeek !== '*') {
      const days = dayOfWeek.split(',').map(d => dayNames[parseInt(d)] || d);
      if (days.length === 7) {
        return `Daily at ${time}`;
      }
      return `${days.join(', ')} at ${time}`;
    }

    // Monthly
    if (dayOfMonth !== '*' && month === '*' && dayOfWeek === '*') {
      return `Monthly on day ${dayOfMonth} at ${time}`;
    }

    return cron;
  }

  function formatNextRun(date: string | null): string {
    if (!date) return 'Not scheduled';
    return new Date(date).toLocaleString();
  }

  // Preview the cron expression
  let cronPreview = $derived(buildCronExpression());
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-gray-900">
      Schedule
      <HelpTooltip text="Set up automatic backup times. You can add multiple schedules (e.g., daily at 2 AM and weekly full backup on Sundays). Each schedule can be individually enabled or disabled. Without a schedule, backups only run when you click 'Run Now'." />
    </h2>
    {#if !showAdd}
      <button onclick={() => (showAdd = true)} class="btn btn-secondary text-sm">
        Add Schedule
      </button>
    {/if}
  </div>

  {#if schedules.length === 0 && !showAdd}
    <p class="text-gray-500 text-sm">No schedules configured. Backups will only run manually.</p>
  {/if}

  <!-- Existing Schedules -->
  {#each schedules as schedule (schedule.id)}
    <div class="flex items-center justify-between p-3 bg-gray-50 rounded-lg">
      <div class="flex items-center gap-3">
        <input
          type="checkbox"
          checked={schedule.enabled}
          onchange={() => toggleSchedule(schedule)}
          class="rounded text-primary-600"
        />
        <div>
          <div class="font-medium text-gray-900">{formatSchedule(schedule)}</div>
          <div class="text-sm text-gray-500">Next: {formatNextRun(schedule.next_run_at)}</div>
        </div>
      </div>
      <button
        onclick={() => deleteSchedule(schedule.id)}
        class="text-red-600 hover:text-red-700 text-sm"
      >
        Delete
      </button>
    </div>
  {/each}

  <!-- Add Schedule Form -->
  {#if showAdd}
    <div class="border rounded-lg p-4 space-y-4">
      <div>
        <label for="scheduleType" class="label">Schedule Type</label>
        <select id="scheduleType" bind:value={scheduleType} class="input">
          <option value="daily">Daily</option>
          <option value="weekly">Weekly (select days)</option>
          <option value="monthly">Monthly</option>
          <option value="interval">Interval (every N minutes/hours)</option>
          <option value="custom">Custom (Cron)</option>
        </select>
      </div>

      {#if scheduleType === 'daily'}
        <div>
          <label for="dailyTime" class="label">Time</label>
          <input id="dailyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
      {/if}

      {#if scheduleType === 'weekly'}
        <div>
          <label for="weeklyTime" class="label">Time</label>
          <input id="weeklyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
        <div>
          <label class="label">
            Days of Week
            <HelpTooltip text="Select one or more days. The backup will run at the specified time on each selected day." />
          </label>
          <div class="flex flex-wrap gap-2 mt-2">
            {#each dayNamesFull as day, index}
              <button
                type="button"
                onclick={() => toggleDay(index)}
                class="px-3 py-1.5 text-sm rounded-lg border transition-colors {selectedDays.includes(index)
                  ? 'bg-primary-600 text-white border-primary-600'
                  : 'bg-white text-gray-700 border-gray-300 hover:border-primary-400'}"
              >
                {day.slice(0, 3)}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if scheduleType === 'monthly'}
        <div>
          <label for="monthlyTime" class="label">Time</label>
          <input id="monthlyTime" type="time" bind:value={timeOfDay} class="input w-32" />
        </div>
        <div>
          <label for="dayOfMonth" class="label">Day of Month</label>
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
              Run every
              <HelpTooltip text="Set how often the backup should run. For example, 'every 30 minutes' or 'every 6 hours'. Note: Very frequent backups may impact system performance." />
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
              <option value="minutes">Minutes</option>
              <option value="hours">Hours</option>
            </select>
          </div>
        </div>
      {/if}

      {#if scheduleType === 'custom'}
        <div>
          <label class="label">
            Cron Expression
            <HelpTooltip text="Advanced scheduling using cron syntax. Five fields: minute (0-59), hour (0-23), day of month (1-31), month (1-12), day of week (0-6, Sun=0). Use * for 'any'. Examples: '0 2 * * *' = 2:00 AM daily, '0 3 * * 0' = 3:00 AM every Sunday, '0 */6 * * *' = every 6 hours." />
          </label>
          <input
            type="text"
            bind:value={cronExpression}
            placeholder="0 2 * * *"
            class="input font-mono"
          />
          <p class="text-sm text-gray-500 mt-1">
            Format: minute hour day-of-month month day-of-week
          </p>
        </div>
      {/if}

      <!-- Cron Preview -->
      {#if scheduleType !== 'custom'}
        <div class="bg-gray-50 rounded-lg p-3">
          <div class="text-xs text-gray-500 uppercase tracking-wide mb-1">Cron Expression</div>
          <code class="text-sm text-gray-800 font-mono">{cronPreview}</code>
        </div>
      {/if}

      <div class="flex gap-2">
        <button onclick={addSchedule} disabled={saving} class="btn btn-primary">
          {saving ? 'Adding...' : 'Add Schedule'}
        </button>
        <button
          onclick={() => {
            showAdd = false;
            resetForm();
          }}
          class="btn btn-secondary"
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}
</div>

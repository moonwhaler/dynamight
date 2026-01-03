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
  let scheduleType = $state<'daily' | 'weekly' | 'monthly' | 'custom'>('daily');
  let timeOfDay = $state('02:00');
  let dayOfWeek = $state(0);
  let dayOfMonth = $state(1);
  let cronExpression = $state('0 2 * * *');

  async function addSchedule() {
    saving = true;
    try {
      const request: CreateScheduleRequest =
        scheduleType === 'custom'
          ? { cron_expression: cronExpression }
          : { schedule_type: scheduleType, time_of_day: timeOfDay, day_of_week: dayOfWeek, day_of_month: dayOfMonth };

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
    dayOfWeek = 0;
    dayOfMonth = 1;
    cronExpression = '0 2 * * *';
  }

  function formatSchedule(schedule: Schedule): string {
    if (schedule.schedule_type === 'daily') {
      return `Daily at ${schedule.time_of_day || '00:00'}`;
    }
    if (schedule.schedule_type === 'weekly') {
      const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
      return `Weekly on ${days[schedule.day_of_week || 0]} at ${schedule.time_of_day || '00:00'}`;
    }
    if (schedule.schedule_type === 'monthly') {
      return `Monthly on day ${schedule.day_of_month || 1} at ${schedule.time_of_day || '00:00'}`;
    }
    return schedule.cron_expression;
  }

  function formatNextRun(date: string | null): string {
    if (!date) return 'Not scheduled';
    return new Date(date).toLocaleString();
  }
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
        <label class="label">Schedule Type</label>
        <select bind:value={scheduleType} class="input">
          <option value="daily">Daily</option>
          <option value="weekly">Weekly</option>
          <option value="monthly">Monthly</option>
          <option value="custom">Custom (Cron)</option>
        </select>
      </div>

      {#if scheduleType !== 'custom'}
        <div>
          <label class="label">Time</label>
          <input type="time" bind:value={timeOfDay} class="input w-32" />
        </div>

        {#if scheduleType === 'weekly'}
          <div>
            <label class="label">Day of Week</label>
            <select bind:value={dayOfWeek} class="input">
              <option value={0}>Sunday</option>
              <option value={1}>Monday</option>
              <option value={2}>Tuesday</option>
              <option value={3}>Wednesday</option>
              <option value={4}>Thursday</option>
              <option value={5}>Friday</option>
              <option value={6}>Saturday</option>
            </select>
          </div>
        {/if}

        {#if scheduleType === 'monthly'}
          <div>
            <label class="label">Day of Month</label>
            <input
              type="number"
              bind:value={dayOfMonth}
              min="1"
              max="31"
              class="input w-20"
            />
          </div>
        {/if}
      {:else}
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

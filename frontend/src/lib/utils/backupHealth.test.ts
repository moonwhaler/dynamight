import { describe, it, expect } from 'vitest';
import { jobHealth, nextScheduledRun, oldestBackup } from './backupHealth';
import type { Job, Schedule } from '../types';

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const NOW = Date.parse('2026-11-24T12:00:00Z');

function ago(ms: number): string {
  return new Date(NOW - ms).toISOString();
}

function schedule(cron: string, extra: Partial<Schedule> = {}): Schedule {
  return { id: 1, job_id: 1, enabled: true, cron_expression: cron, next_run_at: null, ...extra } as Schedule;
}

function job(extra: Partial<Job> = {}): Job {
  return {
    id: 1,
    name: 'job',
    enabled: true,
    created_at: ago(100 * DAY),
    last_run_status: 'completed',
    last_success_at: ago(HOUR),
    schedules: [schedule('0 2 * * *')],
    // What the server sends for a daily schedule: 2 days + 1 hour
    overdue_after_secs: (2 * DAY + HOUR) / 1000,
    ...extra,
  } as Job;
}

describe('jobHealth', () => {
  it('reports a failed last run', () => {
    expect(jobHealth(job({ last_run_status: 'failed' }), NOW).state).toBe('failed');
  });

  it('tolerates one missed daily run but not two', () => {
    expect(jobHealth(job({ last_success_at: ago(DAY + 2 * HOUR) }), NOW).state).toBe('ok');
    expect(jobHealth(job({ last_success_at: ago(3 * DAY) }), NOW)).toEqual({
      state: 'overdue',
      lastSuccessAt: ago(3 * DAY),
    });
  });

  it('gives a new job time for its first run', () => {
    expect(jobHealth(job({ last_success_at: null, created_at: ago(HOUR) }), NOW).state).toBe('ok');
    expect(jobHealth(job({ last_success_at: null }), NOW)).toEqual({ state: 'overdue', lastSuccessAt: null });
  });

  it('expects nothing of disabled jobs or jobs without an overdue limit', () => {
    expect(jobHealth(job({ enabled: false, last_success_at: null }), NOW).state).toBe('ok');
    expect(jobHealth(job({ overdue_after_secs: null, last_success_at: null }), NOW).state).toBe('ok');
  });

  it('follows the limit the server sends', () => {
    const weekly = (8 * DAY) / 1000;
    expect(jobHealth(job({ overdue_after_secs: weekly, last_success_at: ago(5 * DAY) }), NOW).state).toBe('ok');
    expect(jobHealth(job({ overdue_after_secs: weekly, last_success_at: ago(9 * DAY) }), NOW).state).toBe('overdue');
  });
});

describe('nextScheduledRun / oldestBackup', () => {
  it('picks the soonest enabled schedule of an enabled job', () => {
    const a = job({ id: 1, schedules: [schedule('0 2 * * *', { next_run_at: '2026-11-25T02:00:00Z' })] });
    const b = job({ id: 2, schedules: [schedule('0 1 * * *', { next_run_at: '2026-11-25T01:00:00Z' })] });
    const off = job({ id: 3, enabled: false, schedules: [schedule('0 0 * * *', { next_run_at: '2026-11-24T13:00:00Z' })] });
    expect(nextScheduledRun([a, b, off])).toEqual({ job: b, at: '2026-11-25T01:00:00Z' });
    expect(nextScheduledRun([])).toBeNull();
  });

  it('treats a scheduled job without success as the oldest', () => {
    const a = job({ id: 1, last_success_at: ago(2 * DAY) });
    const b = job({ id: 2, last_success_at: ago(5 * DAY) });
    expect(oldestBackup([a, b])?.job).toBe(b);
    const never = job({ id: 3, last_success_at: null });
    expect(oldestBackup([a, never, b])).toEqual({ job: never, at: null });
    expect(oldestBackup([job({ schedules: [] })])).toBeNull();
  });
});

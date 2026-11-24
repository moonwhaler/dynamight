import { describe, it, expect } from 'vitest';
import { jobStatus, matchesFilter, runDurationMs, upcomingRuns, runsByJob } from './jobStatus';
import type { Job, JobRun, Schedule } from '../types';

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const NOW = Date.parse('2026-11-24T12:00:00Z');
const ago = (ms: number) => new Date(NOW - ms).toISOString();

function job(extra: Partial<Job> = {}): Job {
  return {
    id: 1,
    name: 'job',
    enabled: true,
    created_at: ago(100 * DAY),
    last_run_status: 'completed',
    last_success_at: ago(HOUR),
    schedules: [{ id: 1, job_id: 1, enabled: true, cron_expression: '0 2 * * *', next_run_at: null } as Schedule],
    overdue_after_secs: (2 * DAY + HOUR) / 1000,
    ...extra,
  } as Job;
}

describe('jobStatus', () => {
  it('reports a current job as ok', () => {
    expect(jobStatus(job(), NOW)).toEqual({ state: 'ok', attention: false, lastSuccessAt: ago(HOUR) });
  });

  it('puts disabled before everything else', () => {
    expect(jobStatus(job({ enabled: false, last_run_status: 'failed' }), NOW).state).toBe('disabled');
  });

  it('shows running and pending runs as running', () => {
    expect(jobStatus(job({ last_run_status: 'running' }), NOW).state).toBe('running');
    expect(jobStatus(job({ last_run_status: 'pending' }), NOW).state).toBe('running');
  });

  it('flags failed and overdue jobs', () => {
    expect(jobStatus(job({ last_run_status: 'failed' }), NOW)).toMatchObject({ state: 'failed', attention: true });
    expect(jobStatus(job({ last_success_at: ago(5 * DAY) }), NOW)).toMatchObject({ state: 'overdue', attention: true });
  });

  it('separates a job that never succeeded from an overdue one', () => {
    // Created long ago, scheduled, never succeeded: needs attention
    expect(jobStatus(job({ last_success_at: null, last_run_status: null }), NOW)).toMatchObject({
      state: 'never',
      attention: true,
    });
    // Created an hour ago: still waiting for its first run
    expect(
      jobStatus(job({ last_success_at: null, last_run_status: null, created_at: ago(HOUR) }), NOW),
    ).toMatchObject({ state: 'never', attention: false });
    // Manual job that never ran: nothing expected
    expect(
      jobStatus(job({ last_success_at: null, last_run_status: null, overdue_after_secs: null }), NOW),
    ).toMatchObject({ state: 'never', attention: false });
  });

  it('matches the list filters', () => {
    const never = jobStatus(job({ last_success_at: null, overdue_after_secs: null }), NOW);
    expect(matchesFilter(never, 'ok')).toBe(true);
    expect(matchesFilter(never, 'attention')).toBe(false);
    expect(matchesFilter(jobStatus(job({ last_run_status: 'failed' }), NOW), 'attention')).toBe(true);
    expect(matchesFilter(jobStatus(job({ enabled: false }), NOW), 'disabled')).toBe(true);
  });
});

describe('run helpers', () => {
  it('measures durations, also of running runs', () => {
    expect(runDurationMs({ started_at: ago(HOUR), completed_at: ago(0) }, NOW)).toBe(HOUR);
    expect(runDurationMs({ started_at: ago(HOUR), completed_at: null }, NOW)).toBe(HOUR);
    expect(runDurationMs({ started_at: null, completed_at: null }, NOW)).toBeNull();
  });

  it('lists upcoming runs of enabled schedules, soonest first', () => {
    const s = (id: number, at: string | null, enabled = true) =>
      ({ id, job_id: 1, enabled, cron_expression: '* * * * *', next_run_at: at }) as Schedule;
    const a = job({ id: 1, schedules: [s(1, '2026-11-25T02:00:00Z'), s(2, '2026-11-24T20:00:00Z'), s(3, null)] });
    const b = job({ id: 2, schedules: [s(4, '2026-11-24T13:00:00Z', false)] });
    const off = job({ id: 3, enabled: false, schedules: [s(5, '2026-11-24T12:30:00Z')] });
    expect(upcomingRuns([a, b, off]).map((u) => u.scheduleId)).toEqual([2, 1]);
  });

  it('groups runs by job in input order', () => {
    const r = (id: number, job_id: number) => ({ id, job_id }) as JobRun;
    const map = runsByJob([r(3, 1), r(2, 2), r(1, 1)]);
    expect(map.get(1)?.map((x) => x.id)).toEqual([3, 1]);
    expect(map.get(2)?.map((x) => x.id)).toEqual([2]);
  });
});

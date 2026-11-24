import type { Job } from '../types';
import { describeCron } from './cron';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Slack for long runs, the scheduler's one-minute tick and clock skew */
const GRACE_MS = HOUR;

/**
 * Longest time between two runs of a cron schedule, or null when the
 * expression is too unusual to tell (then no overdue warning is given).
 */
export function maxScheduleGapMs(cron: string): number | null {
  const d = describeCron(cron);
  switch (d.kind) {
    case 'every_minutes':
      return Math.min(d.count, 60) * MINUTE;
    case 'every_hours':
      return Math.min(d.count, 24) * HOUR;
    case 'daily':
      return DAY;
    case 'days': {
      // Largest distance between neighbouring weekdays, across the week end
      let gap = 0;
      d.days.forEach((day, i) => {
        const next = i + 1 < d.days.length ? d.days[i + 1] : d.days[0] + 7;
        gap = Math.max(gap, next - day);
      });
      return gap * DAY;
    }
    case 'monthly':
      // Day 29-31 is skipped in shorter months
      return (d.day > 28 ? 62 : 31) * DAY;
    default:
      return null;
  }
}

export type JobHealth =
  /** Nothing to report */
  | { state: 'ok' }
  /** The last run failed */
  | { state: 'failed' }
  /** No successful backup for longer than the schedule allows */
  | { state: 'overdue'; lastSuccessAt: string | null };

/**
 * Whether a job needs attention.
 *
 * A job is overdue when it is scheduled and the last successful backup (or,
 * if there never was one, the job's creation) lies more than twice the
 * longest schedule gap in the past: one missed run is tolerated, two are not.
 */
export function jobHealth(job: Job, now = Date.now()): JobHealth {
  if (job.last_run_status === 'failed') return { state: 'failed' };
  if (!job.enabled) return { state: 'ok' };

  const gaps = (job.schedules ?? [])
    .filter((s) => s.enabled)
    .map((s) => maxScheduleGapMs(s.cron_expression));
  // The most frequent schedule sets the expectation; unknown ones are ignored
  const known = gaps.filter((g): g is number => g !== null);
  if (known.length === 0) return { state: 'ok' };
  const allowed = 2 * Math.min(...known) + GRACE_MS;

  const since = job.last_success_at ?? job.created_at;
  const sinceMs = Date.parse(since);
  if (Number.isNaN(sinceMs) || now - sinceMs <= allowed) return { state: 'ok' };
  return { state: 'overdue', lastSuccessAt: job.last_success_at ?? null };
}

/** The soonest upcoming run over all enabled schedules of enabled jobs. */
export function nextScheduledRun(jobs: Job[]): { job: Job; at: string } | null {
  let best: { job: Job; at: string } | null = null;
  for (const job of jobs) {
    if (!job.enabled) continue;
    for (const s of job.schedules ?? []) {
      if (!s.enabled || !s.next_run_at) continue;
      if (!best || Date.parse(s.next_run_at) < Date.parse(best.at)) {
        best = { job, at: s.next_run_at };
      }
    }
  }
  return best;
}

/**
 * The scheduled job whose last successful backup is oldest (a job that never
 * succeeded counts as oldest). Unscheduled and disabled jobs are left out:
 * they are not expected to be current.
 */
export function oldestBackup(jobs: Job[]): { job: Job; at: string | null } | null {
  let oldest: { job: Job; at: string | null } | null = null;
  for (const job of jobs) {
    if (!job.enabled || !(job.schedules ?? []).some((s) => s.enabled)) continue;
    const at = job.last_success_at ?? null;
    if (
      !oldest ||
      (oldest.at !== null && (at === null || Date.parse(at) < Date.parse(oldest.at)))
    ) {
      oldest = { job, at };
    }
  }
  return oldest;
}

import type { Job } from '../types';

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
 * Overdue uses the limit the server derives from the job's enabled schedules
 * (`overdue_after_secs`: twice the longest gap of the most frequent schedule
 * plus an hour), the same rule the overdue notifications use. The reference
 * time is the last successful backup, or the job's creation if there was none.
 */
export function jobHealth(job: Job, now = Date.now()): JobHealth {
  if (job.last_run_status === 'failed') return { state: 'failed' };
  if (!job.enabled || !job.overdue_after_secs) return { state: 'ok' };

  const since = job.last_success_at ?? job.created_at;
  const sinceMs = Date.parse(since);
  if (Number.isNaN(sinceMs) || now - sinceMs <= job.overdue_after_secs * 1000) return { state: 'ok' };
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

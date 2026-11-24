import type { Job, JobRun } from '../types';
import { jobHealth } from './backupHealth';

/**
 * One state per job, used for every status chip, filter and the overview.
 *
 * - running:  a run is in progress
 * - failed:   the last run failed
 * - overdue:  the last good backup is older than the schedule allows
 * - never:    no successful backup yet
 * - ok:       a good backup exists and is current (or the job only runs manually)
 * - disabled: switched off; nothing is expected of it
 */
export type JobState = 'running' | 'failed' | 'overdue' | 'never' | 'ok' | 'disabled';

export interface JobStatus {
  state: JobState;
  /** Whether the job should be listed under "needs attention" */
  attention: boolean;
  lastSuccessAt: string | null;
}

export function jobStatus(job: Job, now = Date.now()): JobStatus {
  const lastSuccessAt = job.last_success_at ?? null;
  if (!job.enabled) return { state: 'disabled', attention: false, lastSuccessAt };
  if (job.last_run_status === 'running' || job.last_run_status === 'pending') {
    return { state: 'running', attention: false, lastSuccessAt };
  }
  const health = jobHealth(job, now);
  if (health.state === 'failed') return { state: 'failed', attention: true, lastSuccessAt };
  if (health.state === 'overdue') {
    return { state: lastSuccessAt ? 'overdue' : 'never', attention: true, lastSuccessAt };
  }
  // Not overdue: either current, or new and still within its first interval
  return { state: lastSuccessAt ? 'ok' : 'never', attention: false, lastSuccessAt };
}

/** Order for "most urgent first" sorting */
export const STATE_ORDER: Record<JobState, number> = {
  failed: 0,
  overdue: 1,
  never: 2,
  running: 3,
  ok: 4,
  disabled: 5,
};

/** Filter groups offered in the job list */
export type StateFilter = 'attention' | 'running' | 'ok' | 'disabled';

export function matchesFilter(status: JobStatus, filter: StateFilter): boolean {
  switch (filter) {
    case 'attention':
      return status.attention;
    case 'running':
      return status.state === 'running';
    case 'ok':
      return status.state === 'ok' || (status.state === 'never' && !status.attention);
    case 'disabled':
      return status.state === 'disabled';
  }
}

/** Duration of a run in milliseconds, or null while unknown */
export function runDurationMs(run: Pick<JobRun, 'started_at' | 'completed_at'>, now = Date.now()): number | null {
  if (!run.started_at) return null;
  const start = Date.parse(run.started_at);
  const end = run.completed_at ? Date.parse(run.completed_at) : now;
  if (Number.isNaN(start) || Number.isNaN(end)) return null;
  return Math.max(0, end - start);
}

/** Whether a run moved data worth showing (failed/cancelled runs show "—") */
export function runHasResult(run: Pick<JobRun, 'status'>): boolean {
  return run.status === 'completed' || run.status === 'running';
}

/** Enabled schedules of a job with their next run time, soonest first */
export function upcomingRuns(jobs: Job[]): { job: Job; scheduleId: number; at: string; cron: string }[] {
  const out: { job: Job; scheduleId: number; at: string; cron: string }[] = [];
  for (const job of jobs) {
    if (!job.enabled) continue;
    for (const s of job.schedules ?? []) {
      if (s.enabled && s.next_run_at) out.push({ job, scheduleId: s.id, at: s.next_run_at, cron: s.cron_expression });
    }
  }
  return out.sort((a, b) => Date.parse(a.at) - Date.parse(b.at));
}

/** Group runs by job ID, keeping the input order (newest first) */
export function runsByJob(runs: JobRun[]): Map<number, JobRun[]> {
  const map = new Map<number, JobRun[]>();
  for (const run of runs) {
    const list = map.get(run.job_id);
    if (list) list.push(run);
    else map.set(run.job_id, [run]);
  }
  return map;
}

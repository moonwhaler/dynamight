import type { Job, CreateJobRequest, Schedule } from '$lib/types';
import { destinationKind, destinationTarget } from '$lib/utils/jobUtils';
import { destinationKindLabel } from '$lib/utils/destination';
import { formatCron } from './schedule';
import { shortenPath } from '$lib/utils/path';
import * as m from '$lib/paraglide/messages.js';

type JobLike = Pick<Job, 'source_dirs' | 'destination' | 'destination_type' | 'sync_options'> | CreateJobRequest;

function lastSegment(path: string): string {
  const parts = path.replace(/\/+$/, '').split('/');
  return parts[parts.length - 1] || path;
}

/**
 * One plain sentence about what a job does, e.g.
 * "Copies photos to /mnt/backup/photos (USB drive). Schedule: Daily at 02:00.
 * Files deleted at the source stay in the backup."
 */
export function jobSummary(job: JobLike, schedules: Pick<Schedule, 'enabled' | 'cron_expression'>[] | string[]): string {
  const sources = job.source_dirs ?? [];
  const what =
    sources.length === 0
      ? m.summary_nothing_yet()
      : sources.length === 1
        ? lastSegment(sources[0])
        : m.jobs_folders({ count: sources.length });

  const dest = job.destination
    ? `${shortenPath(destinationTarget({ destination: job.destination }), 40)} (${destinationKindLabel(destinationKind({ destination: job.destination, destination_type: job.destination_type ?? job.destination.type }))})`
    : '';

  const crons = schedules
    .filter((s) => typeof s === 'string' || s.enabled)
    .map((s) => (typeof s === 'string' ? s : s.cron_expression));
  const when = crons.length === 0 ? m.summary_when_manual() : m.summary_schedule({ when: crons.map(formatCron).join('; ') });

  const mirror = job.sync_options?.delete_extraneous;
  const sentence = m.summary_sentence({ what, dest: dest.trim() });
  return `${sentence} ${when} ${mirror ? m.summary_mirror() : m.summary_keeps_deleted()}`;
}

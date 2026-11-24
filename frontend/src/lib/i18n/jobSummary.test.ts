import { describe, it, expect } from 'vitest';
import { jobSummary } from './jobSummary';
import type { Job } from '../types';

const job = {
  source_dirs: ['/srv/photos'],
  destination_type: 'local',
  destination: { type: 'local', mount_point: '/mnt/usb', backup_subdir: 'b', usb_uuid: 'abc', auto_mount: true, auto_unmount: true },
  sync_options: { delete_extraneous: false },
} as unknown as Job;

describe('jobSummary', () => {
  it('names the folder, the destination and the schedule', () => {
    expect(jobSummary(job, [{ enabled: true, cron_expression: '0 2 * * *' }])).toBe(
      'summary_sentence(what=photos,dest=/mnt/usb/b (dest_kind_usb)) summary_schedule(when=schedule_daily_at(time=02:00)) summary_keeps_deleted',
    );
  });

  it('counts several folders, skips paused schedules and mentions mirror mode', () => {
    const many = { ...job, source_dirs: ['/a', '/b'], sync_options: { delete_extraneous: true } } as unknown as Job;
    expect(jobSummary(many, [{ enabled: false, cron_expression: '0 2 * * *' }])).toBe(
      'summary_sentence(what=jobs_folders(count=2),dest=/mnt/usb/b (dest_kind_usb)) summary_when_manual summary_mirror',
    );
  });

  it('accepts plain cron strings (drafts)', () => {
    expect(jobSummary(job, ['0 2 * * *', '0 14 * * *'])).toContain('summary_schedule(when=schedule_daily_at(time=02:00); schedule_daily_at(time=14:00))');
  });
});

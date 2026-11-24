import { describe, it, expect, vi } from 'vitest';

vi.mock('./api', () => ({
  api: {
    schedules: {
      delete: vi.fn().mockResolvedValue({}),
      update: vi.fn().mockResolvedValue({}),
      create: vi.fn().mockRejectedValueOnce(new Error('nope')).mockResolvedValue({}),
    },
  },
}));

import { JobDraft, cloneName } from './jobDraft.svelte';
import { api } from './api';
import type { Job, Schedule } from './types';

const job = {
  id: 7,
  name: 'Photos',
  description: null,
  enabled: true,
  source_dirs: ['/srv/photos'],
  destination_type: 'local',
  destination: { type: 'local', mount_point: '/mnt/usb', backup_subdir: 'b', usb_uuid: 'u1', auto_mount: true, auto_unmount: false },
  sync_options: {
    delete_extraneous: true,
    exclude_patterns: ['*.tmp'],
    exclude_dirs: [],
    bandwidth_limit_kbps: null,
    dry_run: false,
    verbosity: 'normal',
    provider_options: { checksum_mode: true },
    space_check: 'warn',
  },
  credential_id: null,
} as unknown as Job;

const sched = (id: number, enabled = true) => ({ id, job_id: 7, enabled, cron_expression: `0 ${id} * * *` }) as Schedule;

describe('JobDraft', () => {
  it('round-trips a job into a request, with the legacy local fields', () => {
    const d = new JobDraft();
    d.load(job, []);
    const r = d.toRequest();
    expect(r).toMatchObject({
      name: 'Photos',
      source_dirs: ['/srv/photos'],
      destination_type: 'local',
      mount_point: '/mnt/usb',
      usb_uuid: 'u1',
      auto_mount: true,
      auto_unmount: false,
      sync_deletes: true,
      rsync_excludes: ['*.tmp'],
      checksum_mode: true,
    });
    expect(r.description).toBeUndefined();
  });

  it('is clean after loading and dirty after a change or schedule edit', () => {
    const d = new JobDraft();
    d.load(job, [sched(1)]);
    expect(d.dirty).toBe(false);
    d.name = 'Pictures';
    expect(d.dirty).toBe(true);
    d.name = 'Photos';
    expect(d.dirty).toBe(false);
    d.schedules[0].enabled = false;
    expect(d.schedulesDirty).toBe(true);
    expect(d.dirty).toBe(true);
  });

  it('does not share nested objects with the loaded job', () => {
    const d = new JobDraft();
    d.load(job, []);
    d.syncOptions.exclude_patterns.push('*.bak');
    expect(job.sync_options.exclude_patterns).toEqual(['*.tmp']);
  });

  it('validates the parts in form order', () => {
    const d = new JobDraft();
    expect(d.firstInvalid()?.section).toBe('general');
    d.name = 'x';
    expect(d.firstInvalid()?.section).toBe('sources');
    d.sourceDirs = ['/a'];
    expect(d.firstInvalid()).toBeNull();
    d.setDestinationType('s3');
    expect(d.firstInvalid()?.section).toBe('destination');
    d.credentialId = 3;
    expect(d.firstInvalid()).toBeNull();
  });

  it('applies schedule changes and counts failures', async () => {
    const d = new JobDraft();
    d.load(job, [sched(1), sched(2)]);
    d.schedules = d.schedules.filter((s) => s.id !== 1);
    d.schedules[0].enabled = false;
    d.pendingSchedules = ['0 5 * * *', '0 6 * * *'];
    expect(await d.saveSchedules(7)).toBe(1);
    expect(api.schedules.delete).toHaveBeenCalledWith(1);
    expect(api.schedules.update).toHaveBeenCalledWith(2, { enabled: false });
    expect(api.schedules.create).toHaveBeenCalledTimes(2);
  });

  it('starts a clone unsaved, with enabled schedules as drafts', () => {
    const d = new JobDraft();
    d.loadClone(job, [sched(1), sched(2, false)], 'Photos (clone)');
    expect(d.name).toBe('Photos (clone)');
    expect(d.pendingSchedules).toEqual(['0 1 * * *']);
    expect(d.dirty).toBe(true);
  });
});

describe('cloneName', () => {
  it('finds a free name', () => {
    expect(cloneName('Photos', [])).toBe('Photos (clone)');
    expect(cloneName('Photos (clone)', ['Photos (clone)'])).toBe('Photos (clone 2)');
  });
});

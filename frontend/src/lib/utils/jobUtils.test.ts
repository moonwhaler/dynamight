import { describe, it, expect } from 'vitest';
import { getDestinationLabel } from './jobUtils';
import type { Job } from '../types';

function makeJob(dest: Record<string, unknown>): Job {
  return {
    destination: dest,
    mount_point: '/mnt/usb',
    backup_subdir: 'backups',
  } as unknown as Job;
}

describe('getDestinationLabel', () => {
  it('returns local path', () => {
    const job = makeJob({ type: 'local', mount_point: '/mnt/usb', backup_subdir: 'data' });
    expect(getDestinationLabel(job)).toBe('/mnt/usb/data');
  });

  it('returns Google Drive label', () => {
    const job = makeJob({ type: 'google_drive', folder_id: 'abc123' });
    expect(getDestinationLabel(job)).toBe('Google Drive: abc123');
  });

  it('returns OneDrive label', () => {
    const job = makeJob({ type: 'onedrive', folder_path: '/Backups' });
    expect(getDestinationLabel(job)).toBe('OneDrive: /Backups');
  });

  it('returns S3 label', () => {
    const job = makeJob({ type: 's3', bucket: 'my-bucket', prefix: 'backups/' });
    expect(getDestinationLabel(job)).toBe('S3: my-bucket/backups/');
  });

  it('returns SFTP label', () => {
    const job = makeJob({ type: 'sftp', host: 'server.com', remote_path: '/data' });
    expect(getDestinationLabel(job)).toBe('SFTP: server.com:/data');
  });

  it('returns WebDAV label', () => {
    const job = makeJob({ type: 'webdav', remote_path: '/remote/backups' });
    expect(getDestinationLabel(job)).toBe('WebDAV: /remote/backups');
  });

  it('falls back to legacy fields', () => {
    const job = makeJob({ type: 'unknown' });
    expect(getDestinationLabel(job)).toBe('/mnt/usb/backups');
  });
});

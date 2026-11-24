import { describe, it, expect } from 'vitest';
import { destinationKind, destinationTarget } from './jobUtils';
import { shortenPath } from './path';
import type { Job } from '../types';

function makeJob(dest: Record<string, unknown>): Job {
  return { destination: dest, destination_type: dest.type } as unknown as Job;
}

describe('destinationKind', () => {
  it('tells a USB drive from a plain local folder', () => {
    expect(destinationKind(makeJob({ type: 'local', mount_point: '/mnt/usb', usb_uuid: 'abc' }))).toBe('usb');
    expect(destinationKind(makeJob({ type: 'local', mount_point: '/srv', usb_uuid: null }))).toBe('local');
    expect(destinationKind(makeJob({ type: 's3' }))).toBe('s3');
  });
});

describe('destinationTarget', () => {
  it('joins the local path without double slashes', () => {
    expect(destinationTarget(makeJob({ type: 'local', mount_point: '/mnt/usb/', backup_subdir: 'data' }))).toBe('/mnt/usb/data');
    expect(destinationTarget(makeJob({ type: 'local', mount_point: '/mnt/usb', backup_subdir: '' }))).toBe('/mnt/usb');
  });

  it('names the remote place', () => {
    expect(destinationTarget(makeJob({ type: 's3', bucket: 'my-bucket', prefix: 'backups/' }))).toBe('my-bucket/backups/');
    expect(destinationTarget(makeJob({ type: 'sftp', host: 'server.com', remote_path: '/data' }))).toBe('server.com:/data');
    expect(destinationTarget(makeJob({ type: 'webdav', url: 'https://x', remote_path: '/remote' }))).toBe('/remote');
    expect(destinationTarget(makeJob({ type: 'google_drive', folder_id: 'abc123' }))).toBe('abc123');
    expect(destinationTarget(makeJob({ type: 'onedrive', folder_path: '/Backups' }))).toBe('/Backups');
  });
});

describe('shortenPath', () => {
  it('keeps short paths', () => {
    expect(shortenPath('/mnt/backup', 42)).toBe('/mnt/backup');
  });

  it('keeps the start and the end of long paths', () => {
    const p = '/tmp/claude-1000/very/long/nested/path/to/the/photos/2026';
    const s = shortenPath(p, 30);
    expect(s.startsWith('/tmp/…/')).toBe(true);
    expect(s.endsWith('photos/2026')).toBe(true);
    expect(s.length).toBeLessThanOrEqual(30);
  });
});

import type { Job } from '../types';

/** What kind of place a job backs up to, for icons and short labels */
export type DestinationKind = 'usb' | 'local' | 's3' | 'sftp' | 'webdav' | 'google_drive' | 'onedrive';

export function destinationKind(job: Pick<Job, 'destination' | 'destination_type'>): DestinationKind {
  const d = job.destination;
  if (d.type === 'local') return d.usb_uuid ? 'usb' : 'local';
  return d.type;
}

/** The target within the destination: a path, bucket or folder */
export function destinationTarget(job: Pick<Job, 'destination'>): string {
  const d = job.destination;
  switch (d.type) {
    case 'local':
      return [d.mount_point, d.backup_subdir].filter(Boolean).join('/').replace(/\/+/g, '/');
    case 's3':
      return [d.bucket, d.prefix].filter(Boolean).join('/');
    case 'sftp':
      return `${d.host}:${d.remote_path}`;
    case 'webdav':
      return d.remote_path || d.url;
    case 'google_drive':
      return d.folder_id;
    case 'onedrive':
      return d.folder_path;
  }
}

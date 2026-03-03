import type { Job } from '../types';

export function getDestinationLabel(job: Job): string {
  const dest = job.destination;
  if (dest.type === 'local') return `${dest.mount_point}/${dest.backup_subdir}`;
  if (dest.type === 'google_drive') return `Google Drive: ${dest.folder_id}`;
  if (dest.type === 'onedrive') return `OneDrive: ${dest.folder_path}`;
  if (dest.type === 's3') return `S3: ${dest.bucket}/${dest.prefix}`;
  if (dest.type === 'sftp') return `SFTP: ${dest.host}:${dest.remote_path}`;
  if (dest.type === 'webdav') return `WebDAV: ${dest.remote_path}`;
  return job.mount_point ? `${job.mount_point}/${job.backup_subdir}` : '';
}

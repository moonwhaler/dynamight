import type { IconName } from '../../components/ui/Icon.svelte';
import type { DestinationKind } from './jobUtils';
import * as m from '$lib/paraglide/messages.js';

export const DESTINATION_ICON: Record<DestinationKind, IconName> = {
  usb: 'drive',
  local: 'folder',
  s3: 'cloud',
  sftp: 'server',
  webdav: 'globe',
  google_drive: 'cloud',
  onedrive: 'cloud',
};

export function destinationKindLabel(kind: DestinationKind): string {
  switch (kind) {
    case 'usb':
      return m.dest_kind_usb();
    case 'local':
      return m.dest_kind_local();
    case 's3':
      return 'S3';
    case 'sftp':
      return 'SFTP';
    case 'webdav':
      return 'WebDAV';
    case 'google_drive':
      return 'Google Drive';
    case 'onedrive':
      return 'OneDrive';
  }
}

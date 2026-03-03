export interface User {
  id: number;
  username: string;
  totp_enabled: boolean;
}

// TOTP / 2FA types
export interface TotpSetupResponse {
  secret: string;
  qr_code: string;
  otpauth_url: string;
}

export interface TotpEnableResponse {
  success: boolean;
  recovery_codes: string[];
}

export interface TotpStatusResponse {
  enabled: boolean;
  recovery_codes_remaining: number;
}

export interface LoginResponse {
  success?: boolean;
  user?: User;
  requires_totp?: boolean;
  pending_session_id?: string;
}

export interface TotpRecoveryResponse {
  success: boolean;
  user: User;
  codes_remaining: number;
}

// Destination types for multi-provider support
export type DestinationType = 'local' | 'google_drive' | 'onedrive' | 's3' | 'sftp' | 'webdav';

export interface LocalDestinationConfig {
  type: 'local';
  mount_point: string;
  backup_subdir: string;
  usb_uuid?: string | null;
  auto_mount: boolean;
  auto_unmount: boolean;
}

export interface GoogleDriveDestinationConfig {
  type: 'google_drive';
  folder_id: string;
  shared_drive_id?: string | null;
}

export interface OneDriveDestinationConfig {
  type: 'onedrive';
  folder_path: string;
  drive_id?: string | null;
}

export interface S3DestinationConfig {
  type: 's3';
  bucket: string;
  prefix: string;
  region: string;
  endpoint?: string | null;
  storage_class?: string | null;
}

export interface SftpDestinationConfig {
  type: 'sftp';
  host: string;
  port: number;
  username: string;
  remote_path: string;
  key_based_auth: boolean;
  /** SSH host key fingerprint for TOFU-based MITM protection ("SHA256:..."). Set after first successful connection test. */
  host_key_fingerprint?: string | null;
}

export interface WebDavDestinationConfig {
  type: 'webdav';
  url: string;
  remote_path: string;
}

export type DestinationConfig =
  | LocalDestinationConfig
  | GoogleDriveDestinationConfig
  | OneDriveDestinationConfig
  | S3DestinationConfig
  | SftpDestinationConfig
  | WebDavDestinationConfig;

// Directory compression types
export type CompressFormat = 'tar_gz' | 'zip';

export interface CompressDirsOptions {
  enabled: boolean;
  format: CompressFormat;
  store_only?: boolean;
  add_timestamp: boolean;
  custom_name?: string | null;
  max_archives_per_dir?: number | null;
  staging_path: string;
  password?: string | null;
}

// Space check modes for local syncs
export type SpaceCheckMode = 'fail' | 'warn' | 'none';

// Space check response from API
export interface SpaceCheckResponse {
  source_size: number;
  transfer_size: number;
  destination_free: number;
  destination_total: number;
  fits: boolean;
  deficit: number | null;
  sources: SourceSizeInfo[];
}

export interface SourceSizeInfo {
  path: string;
  size: number;
  transfer_size: number;
}

// Sync options (unified across all providers)
export interface SyncOptions {
  delete_extraneous: boolean;
  exclude_patterns: string[];
  exclude_dirs: string[];
  bandwidth_limit_kbps?: number | null;
  dry_run: boolean;
  verbosity: 'quiet' | 'normal' | 'verbose';
  provider_options?: Record<string, unknown> | null;
  space_check?: SpaceCheckMode;
  compress_dirs?: CompressDirsOptions | null;
}

// Provider capabilities
export interface ProviderCapabilities {
  supports_delete: boolean;
  supports_compression: boolean;
  supports_checksum: boolean;
  supports_bandwidth_limit: boolean;
  supports_exclude_patterns: boolean;
  supports_incremental: boolean;
  supports_dry_run: boolean;
  requires_credentials: boolean;
}

// Provider info
export interface ProviderInfo {
  provider_type: string;
  name: string;
  description: string;
  requires_credentials: boolean;
}

// Credentials
export type CredentialProviderType = 'google_drive' | 'onedrive' | 's3' | 'sftp' | 'webdav';

export interface Credential {
  id: number;
  name: string;
  provider_type: CredentialProviderType;
  created_at: string;
  updated_at: string;
}

export interface S3CredentialData {
  type: 's3';
  access_key_id: string;
  secret_access_key: string;
}

export interface SftpCredentialData {
  type: 'sftp';
  password?: string;
  private_key?: string;
  passphrase?: string;
}

export interface WebDavCredentialData {
  type: 'webdav';
  username: string;
  password: string;
}

export interface OAuthCredentialData {
  type: 'oauth';
  access_token: string;
  refresh_token: string;
  expires_at: number;
}

export type CredentialData = S3CredentialData | SftpCredentialData | WebDavCredentialData | OAuthCredentialData;

export interface CreateCredentialRequest {
  name: string;
  provider_type: CredentialProviderType;
  data: CredentialData;
}

export interface CredentialUsage {
  jobs: { id: number; name: string }[];
  count: number;
}

// Job types
export interface Job {
  id: number;
  name: string;
  description: string | null;
  enabled: boolean;

  // Legacy fields (always present for backwards compatibility)
  usb_uuid: string | null;
  mount_point: string;
  auto_mount: boolean;
  auto_unmount: boolean;
  source_dirs: string[];
  backup_subdir: string;
  sync_deletes: boolean;
  rsync_excludes: string[];
  checksum_mode: boolean;
  compress: boolean;
  dry_run: boolean;
  bandwidth_limit: number | null;
  verbosity: 'quiet' | 'normal' | 'verbose';

  // New provider-based fields
  destination_type: DestinationType;
  destination: DestinationConfig;
  sync_options: SyncOptions;
  credential_id: number | null;

  created_at: string;
  updated_at: string;
  last_run_status?: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled' | null;
  last_run_at?: string | null;

  // Destination storage info (updated after job runs)
  dest_storage_free?: number | null;
  dest_storage_total?: number | null;
  dest_storage_updated_at?: string | null;

  schedules?: Schedule[];
}

export interface Schedule {
  id: number;
  job_id: number;
  enabled: boolean;
  cron_expression: string;
  schedule_type: string | null;
  time_of_day: string | null;
  day_of_week: number | null;
  day_of_month: number | null;
  last_run_at: string | null;
  next_run_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface JobRun {
  id: number;
  job_id: number;
  schedule_id: number | null;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  started_at: string | null;
  completed_at: string | null;
  exit_code: number | null;
  files_transferred: number | null;
  bytes_transferred: number | null;
  total_size: number | null;
  error_count: number;
  summary: Record<string, unknown> | null;
}

export interface LogEntry {
  id: number;
  job_run_id: number;
  timestamp: string;
  level: 'debug' | 'info' | 'warning' | 'error';
  message: string;
  source: string | null;
}

export interface PaginatedLogsResponse {
  entries: LogEntry[];
  total: number;
  offset: number;
  limit: number;
  has_more: boolean;
}

export interface UsbDrive {
  uuid: string;
  name: string;
  fstype: string | null;
  size: string | null;
  mountpoint: string | null;
  label: string | null;
  model: string | null;
  available: string | null;
}

export interface DirectoryEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number | null;
  modified: number | null;
  extension: string | null;
}

export interface CreateJobRequest {
  name: string;
  description?: string;
  enabled?: boolean;

  // Legacy fields (for local/rsync destinations)
  usb_uuid?: string;
  mount_point?: string;
  auto_mount?: boolean;
  auto_unmount?: boolean;
  source_dirs: string[];
  backup_subdir?: string;
  sync_deletes?: boolean;
  rsync_excludes?: string[];
  checksum_mode?: boolean;
  compress?: boolean;
  dry_run?: boolean;
  bandwidth_limit?: number;
  verbosity?: 'quiet' | 'normal' | 'verbose';

  // New provider-based fields
  destination_type?: DestinationType;
  destination?: DestinationConfig;
  sync_options?: SyncOptions;
  credential_id?: number;
}

export interface CreateScheduleRequest {
  enabled?: boolean;
  cron_expression?: string;
  schedule_type?: 'daily' | 'weekly' | 'monthly' | 'custom';
  time_of_day?: string;
  day_of_week?: number;
  day_of_month?: number;
}

// Helper function to create default destination config
export function createDefaultDestination(type: DestinationType): DestinationConfig {
  switch (type) {
    case 'local':
      return {
        type: 'local',
        mount_point: '/mnt/backup',
        backup_subdir: 'backups',
        usb_uuid: null,
        auto_mount: true,
        auto_unmount: true,
      };
    case 'google_drive':
      return {
        type: 'google_drive',
        folder_id: '',
        shared_drive_id: null,
      };
    case 'onedrive':
      return {
        type: 'onedrive',
        folder_path: '/Backups',
        drive_id: null,
      };
    case 's3':
      return {
        type: 's3',
        bucket: '',
        prefix: 'backups/',
        region: 'us-east-1',
        endpoint: null,
        storage_class: null,
      };
    case 'sftp':
      return {
        type: 'sftp',
        host: '',
        port: 22,
        username: '',
        remote_path: '/backups',
        key_based_auth: false,
      };
    case 'webdav':
      return {
        type: 'webdav',
        url: '',
        remote_path: '/backups',
      };
  }
}

// Helper function to create default sync options
export function createDefaultSyncOptions(): SyncOptions {
  return {
    delete_extraneous: false,
    exclude_patterns: [],
    exclude_dirs: [],
    bandwidth_limit_kbps: null,
    dry_run: false,
    verbosity: 'normal',
    provider_options: null,
    space_check: 'warn',
  };
}

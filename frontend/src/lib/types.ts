export interface User {
  id: number;
  username: string;
}

export interface Job {
  id: number;
  name: string;
  description: string | null;
  enabled: boolean;
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
  created_at: string;
  updated_at: string;
  last_run_status?: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled' | null;
  last_run_at?: string | null;
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
}

export interface DirectoryEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number | null;
}

export interface CreateJobRequest {
  name: string;
  description?: string;
  enabled?: boolean;
  usb_uuid?: string;
  mount_point: string;
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
}

export interface CreateScheduleRequest {
  enabled?: boolean;
  cron_expression?: string;
  schedule_type?: 'daily' | 'weekly' | 'monthly' | 'custom';
  time_of_day?: string;
  day_of_week?: number;
  day_of_month?: number;
}

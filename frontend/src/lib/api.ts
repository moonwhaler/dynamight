import type {
  User,
  Job,
  Schedule,
  JobRun,
  LogEntry,
  PaginatedLogsResponse,
  UsbDrive,
  DirectoryEntry,
  CreateJobRequest,
  CreateScheduleRequest,
  LoginResponse,
  TotpSetupResponse,
  TotpEnableResponse,
  TotpStatusResponse,
  TotpRecoveryResponse,
  Credential,
  CreateCredentialRequest,
  CredentialData,
  CredentialUsage,
  ProviderInfo,
  ProviderCapabilities,
  CredentialProviderType,
  DestinationConfig,
  SpaceCheckResponse,
  ImportPreview,
  ImportResult,
} from './types';
import * as m from '$lib/paraglide/messages.js';

const API_BASE = '/api';

export interface RunsQuery {
  limit?: number;
  /** Only runs with a smaller id (next page) */
  before?: number;
  jobId?: number;
  status?: string[];
  /** RFC 3339 times limiting started_at */
  from?: string;
  to?: string;
}

// Error response from the backend with error code
interface ApiErrorResponse {
  code: string;
  params?: Record<string, string | number>;
}

// Legacy error response format
interface LegacyErrorResponse {
  error: string;
}

class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
    public params?: Record<string, string | number>
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/**
 * Translate an error code from the backend to a localized message
 */
function translateErrorCode(code: string, params?: Record<string, string | number>): string {
  // Map error codes to translation functions
  const translations: Record<string, () => string> = {
    // Authentication errors
    INVALID_CREDENTIALS: () => m.error_invalid_credentials(),
    NOT_AUTHENTICATED: () => m.error_not_authenticated(),
    SESSION_EXPIRED: () => m.error_session_expired(),
    TOKEN_INVALID: () => m.error_token_invalid(),
    USER_NOT_FOUND: () => m.error_user_not_found(),
    RATE_LIMITED: () => m.error_rate_limited({ seconds: params?.seconds ?? 60 }),

    // TOTP errors
    TOTP_INVALID_CODE: () => m.error_totp_invalid_code(),
    TOTP_NOT_ENABLED: () => m.error_totp_not_enabled(),

    // Password errors
    PASSWORD_TOO_SHORT: () => m.error_password_too_short(),
    PASSWORD_INCORRECT: () => m.error_password_incorrect(),
    USERNAME_TOO_SHORT: () => m.error_field_required({ field: 'Username' }),

    // Setup errors
    SETUP_ALREADY_DONE: () => m.error_setup_already_done(),

    // Job errors
    JOB_NOT_FOUND: () => m.error_job_not_found(),
    JOB_ALREADY_RUNNING: () => m.error_job_already_running(),
    JOB_NAME_EXISTS: () => m.error_job_name_exists(),
    NO_RUNNING_JOB: () => m.error_no_running_job(),
    JOB_CREATE_FAILED: () => m.job_error_save(),
    JOB_UPDATE_FAILED: () => m.job_error_save(),
    JOB_DELETE_FAILED: () => m.job_error_delete(),
    JOB_CLONE_FAILED: () => m.job_error_clone(),

    // Schedule errors
    SCHEDULE_NOT_FOUND: () => m.error_schedule_not_found(),
    INVALID_CRON: () => m.error_invalid_cron(),

    // Credential errors
    CREDENTIAL_NOT_FOUND: () => m.error_credential_not_found(),
    CREDENTIAL_IN_USE: () => m.error_credential_in_use(),
    CREDENTIAL_CREATE_FAILED: () => m.error_credential_create_failed(),
    CREDENTIAL_UPDATE_FAILED: () => m.error_credential_update_failed(),
    CREDENTIAL_DELETE_FAILED: () => m.error_credential_delete_failed(),

    // Validation errors
    VALIDATION_FIELD_REQUIRED: () => m.error_field_required({ field: String(params?.field ?? 'Field') }),
    VALIDATION_FIELD_TOO_LONG: () => m.error_field_too_long({ field: String(params?.field ?? 'Field'), max: params?.max ?? 255 }),
    SOURCE_DIRS_REQUIRED: () => m.error_source_dirs_required(),
    SOURCE_DIRS_DUPLICATE_BASENAMES: () => {
      const duplicates = params?.duplicates;
      const names = Array.isArray(duplicates) ? duplicates.join(', ') : String(duplicates ?? '');
      return m.error_source_dirs_duplicate_basenames({ names });
    },
    EXCLUDE_DIR_NOT_IN_SOURCE: () => m.error_exclude_dir_not_in_source(),
    CREDENTIALS_REQUIRED: () => m.job_validation_credentials_required(),
    VALIDATION_INVALID_PATTERN: () =>
      m.error_invalid_pattern({ pattern: String(params?.pattern ?? ''), reason: String(params?.reason ?? '') }),

    // Compression validation errors
    COMPRESS_STAGING_PATH_REQUIRED: () => m.compress_dirs_staging_path_required(),
    COMPRESS_STAGING_PATH_NOT_ALLOWED: () => m.error_compress_staging_not_allowed({ path: String(params?.path ?? '') }),
    COMPRESS_STAGING_OVERLAPS_SOURCE: () => m.error_compress_staging_overlaps_source({ path: String(params?.path ?? '') }),
    COMPRESS_INVALID_CUSTOM_NAME: () => m.error_compress_invalid_custom_name(),
    COMPRESS_MAX_ARCHIVES_INVALID: () => m.error_compress_max_archives_invalid(),

    // System errors
    PATH_NOT_ALLOWED: () => m.error_path_not_allowed(),
    PATH_TRAVERSAL_NOT_ALLOWED: () => m.error_path_not_allowed(),
    MOUNT_POINT_NOT_ALLOWED: () => m.error_path_not_allowed(),
    INVALID_UUID_FORMAT: () => m.error_invalid_uuid(),
    TOTP_ALREADY_ENABLED: () => m.error_totp_already_enabled(),

    // File Browser errors
    FILE_NOT_FOUND: () => m.error_file_not_found(),
    NOT_A_FILE: () => m.error_not_a_file(),
    FILE_TOO_LARGE: () => m.error_file_too_large(),
    DOWNLOAD_FAILED: () => m.error_download_failed(),
    DELETE_FAILED: () => m.error_delete_failed(),
    DELETE_VERIFICATION_REQUIRED: () => m.error_delete_verification_required(),
    DELETE_VERIFICATION_FAILED: () => m.error_delete_verification_failed(),

    // Search errors
    SEARCH_FAILED: () => m.error_search_failed(),

    // Run errors
    RUN_NOT_FOUND: () => m.error_generic(),

    // Config backup errors
    BACKUP_INVALID_PASSWORD: () => m.backup_invalid_password(),
    BACKUP_INVALID_FORMAT: () => m.backup_invalid_file(),
    BACKUP_UNSUPPORTED_VERSION: () => m.backup_unsupported_version(),
    BACKUP_JOBS_RUNNING: () => m.backup_jobs_running(),
    BACKUP_PASSWORD_TOO_SHORT: () => m.backup_password_too_short(),
  };

  const translator = translations[code];
  if (translator) {
    return translator();
  }

  // Fallback for unknown codes
  console.warn(`Unknown error code: ${code}`);
  return m.error_generic();
}

// Called when the server answers 401 (session expired or revoked). Set by the
// auth store; a callback avoids a circular import between api and auth.
let onUnauthorized: (() => void) | null = null;

export function setUnauthorizedHandler(handler: () => void) {
  onUnauthorized = handler;
}

async function request<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
    credentials: 'include',
  });

  if (!response.ok) {
    if (response.status === 401) {
      onUnauthorized?.();
    }
    const text = await response.text();
    let errorMessage: string = String(m.error_generic());
    let errorCode: string | undefined;
    let errorParams: Record<string, string | number> | undefined;

    try {
      const errorData = JSON.parse(text);

      // Check if it's the new error code format
      if ('code' in errorData) {
        const apiError = errorData as ApiErrorResponse;
        errorCode = apiError.code;
        errorParams = apiError.params;
        errorMessage = String(translateErrorCode(apiError.code, apiError.params));
      }
      // Legacy format with "error" string
      else if ('error' in errorData) {
        const legacyError = errorData as LegacyErrorResponse;
        errorMessage = String(legacyError.error);
      }
      // Plain { message } bodies (e.g. connection test results)
      else if (typeof errorData.message === 'string' && errorData.message) {
        errorMessage = errorData.message;
      }
    } catch {
      console.error(`API Error [${response.status}] ${endpoint}:`, text || '(empty response)');
    }
    throw new ApiError(response.status, errorMessage, errorCode, errorParams);
  }

  const text = await response.text();
  if (!text) {
    return {} as T;
  }
  try {
    return JSON.parse(text);
  } catch {
    console.error(`JSON parse error for ${endpoint}:`, text);
    throw new Error(`Invalid JSON response from ${endpoint}`);
  }
}

export const api = {
  auth: {
    setupRequired: () => request<{ setup_required: boolean }>('/auth/setup-required'),
    setup: (username: string, password: string) =>
      request<{ success: boolean }>('/auth/setup', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      }),
    login: (username: string, password: string) =>
      request<LoginResponse>('/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      }),
    logout: () => request<{ success: boolean }>('/auth/logout', { method: 'POST' }),
    me: () => request<User>('/auth/me'),
    changePassword: (currentPassword: string, newPassword: string) =>
      request<{ success: boolean }>('/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      }),
    // TOTP / 2FA methods
    totpSetup: () => request<TotpSetupResponse>('/auth/totp/setup', { method: 'POST' }),
    totpEnable: (code: string, secret: string) =>
      request<TotpEnableResponse>('/auth/totp/enable', {
        method: 'POST',
        body: JSON.stringify({ code, secret }),
      }),
    totpDisable: (password: string, code: string) =>
      request<{ success: boolean }>('/auth/totp/disable', {
        method: 'POST',
        body: JSON.stringify({ password, code }),
      }),
    totpStatus: () => request<TotpStatusResponse>('/auth/totp/status'),
    totpValidate: (pendingSessionId: string, code: string) =>
      request<LoginResponse>('/auth/totp/validate', {
        method: 'POST',
        body: JSON.stringify({ pending_session_id: pendingSessionId, code }),
      }),
    totpRecovery: (pendingSessionId: string, recoveryCode: string) =>
      request<TotpRecoveryResponse>('/auth/totp/recovery', {
        method: 'POST',
        body: JSON.stringify({ pending_session_id: pendingSessionId, recovery_code: recoveryCode }),
      }),
    // Setup from backup (initial setup only, no auth required)
    setupFromBackup: async (file: File, password: string): Promise<{ success: boolean; username: string }> => {
      const formData = new FormData();
      formData.append('file', file);
      formData.append('password', password);
      const response = await fetch(`${API_BASE}/auth/setup-from-backup`, {
        method: 'POST',
        credentials: 'include',
        body: formData,
      });
      if (!response.ok) {
        const text = await response.text();
        let errorMessage = String(m.error_generic());
        let errorCode: string | undefined;
        let errorParams: Record<string, string | number> | undefined;
        try {
          const errorData = JSON.parse(text);
          if ('code' in errorData) {
            errorCode = errorData.code;
            errorParams = errorData.params;
            errorMessage = String(translateErrorCode(errorData.code, errorData.params));
          }
        } catch { /* ignore */ }
        throw new ApiError(response.status, errorMessage, errorCode, errorParams);
      }
      return response.json();
    },
  },

  jobs: {
    list: () => request<Job[]>('/jobs'),
    get: (id: number) => request<Job>(`/jobs/${id}`),
    create: (job: CreateJobRequest) =>
      request<Job>('/jobs', {
        method: 'POST',
        body: JSON.stringify(job),
      }),
    update: (id: number, job: Partial<CreateJobRequest>) =>
      request<Job>(`/jobs/${id}`, {
        method: 'PUT',
        body: JSON.stringify(job),
      }),
    delete: (id: number) =>
      request<{ success: boolean }>(`/jobs/${id}`, { method: 'DELETE' }),
    run: (id: number) =>
      request<{ runId: number }>(`/jobs/${id}/run`, { method: 'POST' }),
    cancel: (id: number) =>
      request<{ success: boolean; processKilled: boolean }>(
        `/jobs/${id}/cancel`,
        { method: 'POST' }
      ),
    clone: (id: number) =>
      request<Job>(`/jobs/${id}/clone`, { method: 'POST' }),
    checkSpace: (id: number) =>
      request<SpaceCheckResponse>(`/jobs/${id}/check-space`, { method: 'POST' }),
  },

  schedules: {
    list: (jobId: number) => request<Schedule[]>(`/jobs/${jobId}/schedules`),
    create: (jobId: number, schedule: CreateScheduleRequest) =>
      request<Schedule>(`/jobs/${jobId}/schedules`, {
        method: 'POST',
        body: JSON.stringify(schedule),
      }),
    update: (id: number, schedule: Partial<CreateScheduleRequest>) =>
      request<Schedule>(`/schedules/${id}`, {
        method: 'PUT',
        body: JSON.stringify(schedule),
      }),
    delete: (id: number) =>
      request<{ success: boolean }>(`/schedules/${id}`, { method: 'DELETE' }),
    /** Check a cron expression without saving it */
    validate: (cronExpression: string) =>
      request<{ next_run_at: string | null }>('/schedules/validate', {
        method: 'POST',
        body: JSON.stringify({ cron_expression: cronExpression }),
      }),
  },

  runs: {
    list: (jobId: number, limit = 50, offset = 0) =>
      request<JobRun[]>(`/jobs/${jobId}/runs?limit=${limit}&offset=${offset}`),
    /** Runs across all jobs, newest first, optionally filtered */
    recent: (query: RunsQuery = {}) => {
      const params = new URLSearchParams({ limit: String(query.limit ?? 10) });
      if (query.before != null) params.set('before', String(query.before));
      if (query.jobId != null) params.set('job_id', String(query.jobId));
      if (query.status?.length) params.set('status', query.status.join(','));
      if (query.from) params.set('from', query.from);
      if (query.to) params.set('to', query.to);
      return request<JobRun[]>(`/runs?${params}`);
    },
    get: (id: number) => request<JobRun>(`/runs/${id}`),
    logs: (id: number, limit = 500, offset = 0) =>
      request<PaginatedLogsResponse>(`/runs/${id}/logs?limit=${limit}&offset=${offset}`),
    delete: (id: number) =>
      request<{ success: boolean }>(`/runs/${id}`, { method: 'DELETE' }),
    deleteForJob: (jobId: number) =>
      request<{ success: boolean; deleted: number }>(`/jobs/${jobId}/runs`, { method: 'DELETE' }),
    purgeAll: () =>
      request<{ success: boolean; deleted: number }>('/runs', { method: 'DELETE' }),
  },

  settings: {
    get: () => request<{
      max_runs_per_job: number | null;
      delete_verification_window_minutes: number | null;
      search_timeout_seconds: number | null;
      show_directory_sizes: boolean | null;
    }>('/settings'),
    update: (settings: {
      max_runs_per_job?: number | null;
      delete_verification_window_minutes?: number | null;
      search_timeout_seconds?: number | null;
      show_directory_sizes?: boolean;
    }) =>
      request<{ success: boolean }>('/settings', {
        method: 'PUT',
        body: JSON.stringify(settings),
      }),
  },

  system: {
    version: () =>
      request<{ version: string; build: string; date: string }>('/system/version'),
    /** Server facts for display; `timezone` is the zone schedules run in */
    info: () => request<{ timezone: string }>('/system/info'),
    drives: () => request<UsbDrive[]>('/system/drives'),
    browse: (path: string) =>
      request<{ path: string; entries: DirectoryEntry[] }>(
        `/system/browse?path=${encodeURIComponent(path)}`
      ),
    mkdir: (path: string) =>
      request<{ success: boolean; path: string }>('/system/mkdir', {
        method: 'POST',
        body: JSON.stringify({ path }),
      }),
    allowedPaths: () => request<{ paths: string[] }>('/system/allowed-paths'),
    mount: (uuid: string, mountPoint: string) =>
      request<{ success: boolean }>('/system/mount', {
        method: 'POST',
        body: JSON.stringify({ uuid, mount_point: mountPoint }),
      }),
    unmount: (mountPoint: string) =>
      request<{ success: boolean }>('/system/unmount', {
        method: 'POST',
        body: JSON.stringify({ mount_point: mountPoint }),
      }),
    generateMountPoint: (uuid: string, label?: string) =>
      request<{ mount_point: string }>('/system/generate-mount-point', {
        method: 'POST',
        body: JSON.stringify({ uuid, label }),
      }),
    search: (path: string, query: string, maxResults = 200, options?: { signal?: AbortSignal }) =>
      request<{ base_path: string; query: string; results: DirectoryEntry[]; truncated: boolean; timed_out: boolean }>(
        `/system/search?path=${encodeURIComponent(path)}&query=${encodeURIComponent(query)}&max_results=${maxResults}`,
        options?.signal ? { signal: options.signal } : {}
      ),
    // Direct URL for browser-native file download
    downloadUrl: (path: string) =>
      `${API_BASE}/system/download?path=${encodeURIComponent(path)}`,
    // Delete verification and file deletion
    verifyDeleteAccess: (password: string, totpCode?: string) =>
      request<{ verified: boolean; expires_at: number }>('/system/verify-delete-access', {
        method: 'POST',
        body: JSON.stringify({ password, totp_code: totpCode }),
      }),
    deleteFile: (path: string) =>
      request<{ success: boolean; path: string; is_dir: boolean }>('/system/delete', {
        method: 'DELETE',
        body: JSON.stringify({ path }),
      }),
    deleteStatus: () =>
      request<{ verified: boolean; expires_at?: number }>('/system/delete-status'),
    dirSizes: (paths: string[]) =>
      request<{ sizes: Record<string, number> }>('/system/dir-sizes', {
        method: 'POST',
        body: JSON.stringify({ paths }),
      }),
  },

  credentials: {
    list: (provider?: CredentialProviderType) =>
      request<Credential[]>(provider ? `/credentials?provider=${provider}` : '/credentials'),
    get: (id: number) => request<Credential>(`/credentials/${id}`),
    create: (credential: CreateCredentialRequest) =>
      request<Credential>('/credentials', {
        method: 'POST',
        body: JSON.stringify(credential),
      }),
    update: (id: number, credential: Partial<CreateCredentialRequest>) =>
      request<Credential>(`/credentials/${id}`, {
        method: 'PUT',
        body: JSON.stringify(credential),
      }),
    delete: (id: number) =>
      request<{ success: boolean }>(`/credentials/${id}`, { method: 'DELETE' }),
    getUsage: (id: number) =>
      request<CredentialUsage>(`/credentials/${id}/usage`),
  },

  configBackup: {
    export: async (password: string): Promise<Blob> => {
      const response = await fetch(`${API_BASE}/config/export`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ password }),
      });
      if (!response.ok) {
        const text = await response.text();
        let errorMessage = String(m.backup_export_error());
        try {
          const errorData = JSON.parse(text);
          if ('code' in errorData) {
            errorMessage = String(translateErrorCode(errorData.code, errorData.params));
          }
        } catch { /* ignore */ }
        throw new ApiError(response.status, errorMessage);
      }
      return response.blob();
    },

    preview: async (file: File, password: string, strategy: 'merge' | 'replace'): Promise<ImportPreview> => {
      const formData = new FormData();
      formData.append('file', file);
      formData.append('password', password);
      formData.append('strategy', strategy);
      const response = await fetch(`${API_BASE}/config/import/preview`, {
        method: 'POST',
        credentials: 'include',
        body: formData,
      });
      if (!response.ok) {
        const text = await response.text();
        let errorMessage = String(m.backup_import_error());
        try {
          const errorData = JSON.parse(text);
          if ('code' in errorData) {
            errorMessage = String(translateErrorCode(errorData.code, errorData.params));
          }
        } catch { /* ignore */ }
        throw new ApiError(response.status, errorMessage);
      }
      return response.json();
    },

    import: async (file: File, password: string, strategy: 'merge' | 'replace'): Promise<ImportResult> => {
      const formData = new FormData();
      formData.append('file', file);
      formData.append('password', password);
      formData.append('strategy', strategy);
      const response = await fetch(`${API_BASE}/config/import`, {
        method: 'POST',
        credentials: 'include',
        body: formData,
      });
      if (!response.ok) {
        const text = await response.text();
        let errorMessage = String(m.backup_import_error());
        try {
          const errorData = JSON.parse(text);
          if ('code' in errorData) {
            errorMessage = String(translateErrorCode(errorData.code, errorData.params));
          }
        } catch { /* ignore */ }
        throw new ApiError(response.status, errorMessage);
      }
      return response.json();
    },
  },

  providers: {
    list: () => request<ProviderInfo[]>('/providers'),
    capabilities: (type: string) => request<ProviderCapabilities>(`/providers/${type}/capabilities`),
    testConnection: (
      destination: DestinationConfig,
      credentialId: number | null,
      credentialData?: CredentialData
    ) =>
      request<{ success: boolean; message: string; details?: string }>('/providers/test', {
        method: 'POST',
        body: JSON.stringify({
          destination,
          credential_id: credentialId,
          credential_data: credentialData,
        }),
      }),
  },
};

export { ApiError };

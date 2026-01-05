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
} from './types';
import * as m from '$lib/paraglide/messages.js';

const API_BASE = '/api';

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
    CREDENTIALS_REQUIRED: () => m.job_validation_credentials_required(),

    // System errors
    PATH_NOT_ALLOWED: () => m.error_path_not_allowed(),
    PATH_TRAVERSAL_NOT_ALLOWED: () => m.error_path_not_allowed(),

    // File Browser errors
    FILE_NOT_FOUND: () => m.error_file_not_found(),
    NOT_A_FILE: () => m.error_not_a_file(),
    FILE_TOO_LARGE: () => m.error_file_too_large(),
    DOWNLOAD_FAILED: () => m.error_download_failed(),

    // Run errors
    RUN_NOT_FOUND: () => m.error_generic(),
  };

  const translator = translations[code];
  if (translator) {
    return translator();
  }

  // Fallback for unknown codes
  console.warn(`Unknown error code: ${code}`);
  return m.error_generic();
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
      window.location.hash = '#/login';
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
    // Get current JWT token for WebSocket authentication
    getToken: () => request<{ token: string }>('/auth/token'),
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
  },

  runs: {
    list: (jobId: number, limit = 50, offset = 0) =>
      request<JobRun[]>(`/jobs/${jobId}/runs?limit=${limit}&offset=${offset}`),
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
    get: () => request<{ max_runs_per_job: number | null }>('/settings'),
    update: (settings: { max_runs_per_job: number | null }) =>
      request<{ success: boolean }>('/settings', {
        method: 'PUT',
        body: JSON.stringify(settings),
      }),
  },

  system: {
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
    // Direct URL for browser-native file download
    downloadUrl: (path: string) =>
      `${API_BASE}/system/download?path=${encodeURIComponent(path)}`,
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

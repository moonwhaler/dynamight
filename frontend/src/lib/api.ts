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
} from './types';

const API_BASE = '/api';

class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
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
    let errorMessage = 'Request failed';
    try {
      const errorData = JSON.parse(text);
      errorMessage = errorData.error || errorMessage;
    } catch {
      console.error(`API Error [${response.status}] ${endpoint}:`, text || '(empty response)');
    }
    throw new ApiError(response.status, errorMessage);
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
  },
};

export { ApiError };

import type {
  User,
  Job,
  Schedule,
  JobRun,
  LogEntry,
  UsbDrive,
  DirectoryEntry,
  CreateJobRequest,
  CreateScheduleRequest,
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
    login: (username: string, password: string) =>
      request<{ success: boolean; user: User }>('/auth/login', {
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
      request<{ success: boolean }>(`/jobs/${id}/cancel`, { method: 'POST' }),
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
    logs: (id: number, limit = 1000, offset = 0) =>
      request<LogEntry[]>(`/runs/${id}/logs?limit=${limit}&offset=${offset}`),
  },

  system: {
    drives: () => request<UsbDrive[]>('/system/drives'),
    mounts: () => request<{ path: string; device: string; fstype: string }[]>('/system/mounts'),
    browse: (path: string) =>
      request<{ path: string; entries: DirectoryEntry[] }>(
        `/system/browse?path=${encodeURIComponent(path)}`
      ),
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
    mkdir: (path: string) =>
      request<{ success: boolean; path: string }>('/system/mkdir', {
        method: 'POST',
        body: JSON.stringify({ path }),
      }),
    health: () => request<{ status: string; version: string }>('/system/health'),
  },
};

export { ApiError };

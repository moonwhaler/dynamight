import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Job } from '../types';

// Mock the API module
vi.mock('../api', () => ({
  api: {
    jobs: {
      list: vi.fn(),
    },
  },
}));

describe('jobsStore', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('starts with empty state', async () => {
    const { jobsStore } = await import('./jobs');
    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    expect(state.jobs).toEqual([]);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
    unsub();
  });

  it('loads jobs from API', async () => {
    const mockJobs = [{ id: 1, name: 'Job 1' }, { id: 2, name: 'Job 2' }];
    const { api } = await import('../api');
    vi.mocked(api.jobs.list).mockResolvedValue(mockJobs as Job[]);

    const { jobsStore } = await import('./jobs');
    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    await jobsStore.load();
    expect(state.jobs).toEqual(mockJobs);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
    unsub();
  });

  it('handles load error', async () => {
    const { api } = await import('../api');
    vi.mocked(api.jobs.list).mockRejectedValue(new Error('Network error'));

    const { jobsStore } = await import('./jobs');
    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    await jobsStore.load();
    expect(state.error).toBe('Network error');
    expect(state.loading).toBe(false);
    unsub();
  });

  it('updates a job in place', async () => {
    const mockJobs = [{ id: 1, name: 'Job 1' }, { id: 2, name: 'Job 2' }];
    const { api } = await import('../api');
    vi.mocked(api.jobs.list).mockResolvedValue(mockJobs as Job[]);

    const { jobsStore } = await import('./jobs');
    await jobsStore.load();

    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    jobsStore.updateJob({ id: 1, name: 'Updated Job' } as Job);
    expect(state.jobs[0].name).toBe('Updated Job');
    expect(state.jobs[1].name).toBe('Job 2');
    unsub();
  });

  it('adds a job', async () => {
    const { jobsStore } = await import('./jobs');
    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    jobsStore.addJob({ id: 3, name: 'New Job' } as Job);
    expect(state.jobs).toHaveLength(1);
    expect(state.jobs[0].name).toBe('New Job');
    unsub();
  });

  it('removes a job', async () => {
    const mockJobs = [{ id: 1, name: 'Job 1' }, { id: 2, name: 'Job 2' }];
    const { api } = await import('../api');
    vi.mocked(api.jobs.list).mockResolvedValue(mockJobs as Job[]);

    const { jobsStore } = await import('./jobs');
    await jobsStore.load();

    let state = { jobs: [] as Job[], loading: false, error: null as string | null };
    const unsub = jobsStore.subscribe((s) => { state = s; });

    jobsStore.removeJob(1);
    expect(state.jobs).toHaveLength(1);
    expect(state.jobs[0].id).toBe(2);
    unsub();
  });
});

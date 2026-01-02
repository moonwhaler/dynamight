import { writable } from 'svelte/store';
import { api } from '../api';
import type { Job } from '../types';

interface JobsState {
  jobs: Job[];
  loading: boolean;
  error: string | null;
}

function createJobsStore() {
  const { subscribe, set, update } = writable<JobsState>({
    jobs: [],
    loading: false,
    error: null,
  });

  return {
    subscribe,

    async load() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const jobs = await api.jobs.list();
        set({ jobs, loading: false, error: null });
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to load jobs';
        update((s) => ({ ...s, loading: false, error: message }));
      }
    },

    async refresh() {
      try {
        const jobs = await api.jobs.list();
        update((s) => ({ ...s, jobs }));
      } catch {
        // Silent refresh failure
      }
    },

    updateJob(job: Job) {
      update((s) => ({
        ...s,
        jobs: s.jobs.map((j) => (j.id === job.id ? job : j)),
      }));
    },

    addJob(job: Job) {
      update((s) => ({
        ...s,
        jobs: [...s.jobs, job],
      }));
    },

    removeJob(id: number) {
      update((s) => ({
        ...s,
        jobs: s.jobs.filter((j) => j.id !== id),
      }));
    },
  };
}

export const jobsStore = createJobsStore();

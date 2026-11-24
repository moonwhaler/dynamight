import { writable, get } from 'svelte/store';
import { api } from '../api';
import type { Job, JobRun } from '../types';
import { statusStore } from './logs';

interface JobsState {
  jobs: Job[];
  /** Recent runs over all jobs, newest first (for run strips and activity) */
  runs: JobRun[];
  loading: boolean;
  error: string | null;
  /** Whether the first load finished (successfully or not) */
  loaded: boolean;
}

/** Runs kept for the overview: enough for a strip of 20 per job in most setups */
const RECENT_RUNS = 200;
/** Fallback refresh while live; status events trigger refreshes in between */
const FALLBACK_REFRESH_MS = 30_000;
/** Status events often come in bursts (start + finish); refresh once */
const EVENT_DEBOUNCE_MS = 400;

function createJobsStore() {
  const { subscribe, set, update } = writable<JobsState>({
    jobs: [],
    runs: [],
    loading: false,
    error: null,
    loaded: false,
  });

  let liveUsers = 0;
  let stopLive: (() => void) | null = null;

  async function fetchRuns(): Promise<JobRun[] | null> {
    try {
      return await api.runs.recent({ limit: RECENT_RUNS });
    } catch {
      return null;
    }
  }

  const store = {
    subscribe,

    async load() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const [jobs, runs] = await Promise.all([api.jobs.list(), fetchRuns()]);
        update((s) => ({ ...s, jobs, runs: runs ?? s.runs, loading: false, error: null, loaded: true }));
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to load jobs';
        update((s) => ({ ...s, loading: false, error: message, loaded: true }));
      }
    },

    async refresh() {
      try {
        const [jobs, runs] = await Promise.all([api.jobs.list(), fetchRuns()]);
        update((s) => ({ ...s, jobs, runs: runs ?? s.runs, error: null, loaded: true }));
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
        runs: s.runs.filter((r) => r.job_id !== id),
      }));
    },

    /**
     * Keep jobs and recent runs current while a page is shown. Returns the
     * function to call when the page goes away (usable as onMount's return).
     * Refreshes on run start/finish events and every 30 s while visible.
     */
    live(): () => void {
      liveUsers++;
      if (!get({ subscribe }).loaded) store.load();
      if (!stopLive) stopLive = startLive();
      return () => {
        liveUsers = Math.max(0, liveUsers - 1);
        if (liveUsers === 0 && stopLive) {
          stopLive();
          stopLive = null;
        }
      };
    },

    /** Test helper */
    reset() {
      set({ jobs: [], runs: [], loading: false, error: null, loaded: false });
    },
  };

  function startLive(): () => void {
    let debounce: ReturnType<typeof setTimeout> | null = null;
    const soon = () => {
      if (debounce) clearTimeout(debounce);
      debounce = setTimeout(() => {
        debounce = null;
        store.refresh();
      }, EVENT_DEBOUNCE_MS);
    };

    statusStore.connect();
    let started = false;
    let lastSeen: unknown = undefined;
    const unsubStatus = statusStore.subscribe((events) => {
      // The first callback replays history; only react to new events
      const last = events[events.length - 1];
      if (started && last !== lastSeen) soon();
      lastSeen = last;
      started = true;
    });

    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') store.refresh();
    }, FALLBACK_REFRESH_MS);
    const onVisible = () => {
      if (document.visibilityState === 'visible') store.refresh();
    };
    document.addEventListener('visibilitychange', onVisible);

    return () => {
      if (debounce) clearTimeout(debounce);
      clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisible);
      unsubStatus();
      statusStore.disconnect();
    };
  }

  return store;
}

export const jobsStore = createJobsStore();

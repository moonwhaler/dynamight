import { writable } from 'svelte/store';

/** The run whose log is open in the app-wide log viewer, if any */
export const openRun = writable<{ runId: number; jobId: number } | null>(null);

export function showRunLog(runId: number, jobId: number): void {
  openRun.set({ runId, jobId });
}

export function closeRunLog(): void {
  openRun.set(null);
}

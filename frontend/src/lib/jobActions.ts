// Actions on a job, shared by every page that offers them, so a job behaves
// the same from the overview, the list and its own page.
import { get } from 'svelte/store';
import { push } from 'svelte-spa-router';
import { api } from './api';
import type { Job } from './types';
import { jobsStore } from './stores/jobs';
import { preferencesStore } from './stores/preferences';
import { showRunLog } from './stores/runLog';
import { formatBytes } from './utils/format';
import { showToast } from '../components/ui/Toast.svelte';
import { confirm } from '../components/ui/ConfirmDialog.svelte';
import * as m from '$lib/paraglide/messages.js';

export function isRunning(job: Pick<Job, 'last_run_status'>): boolean {
  return job.last_run_status === 'running' || job.last_run_status === 'pending';
}

/** Start a run; opens its log when the preference says so. */
export async function runJob(job: Job): Promise<number | null> {
  try {
    const { runId, queued } = await api.jobs.run(job.id);
    jobsStore.refresh();
    if (get(preferencesStore).showLogViewerAfterManualRun) showRunLog(runId, job.id);
    else if (queued) showToast({ message: m.job_run_queued({ name: job.name }), variant: 'info' });
    else showToast({ message: m.job_run_started({ name: job.name }), variant: 'success' });
    return runId;
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.job_error_start(), variant: 'error' });
    return null;
  }
}

/** Stop the running run, asking first when the preference says so. */
export async function stopJob(job: Job): Promise<boolean> {
  if (get(preferencesStore).confirmKillProcess) {
    const ok = await confirm({
      title: m.kill_confirm_title(),
      message: m.kill_confirm_message(),
      confirmText: m.kill_confirm_button(),
      variant: 'danger',
    });
    if (!ok) return false;
  }
  try {
    await api.jobs.cancel(job.id);
    await jobsStore.refresh();
    return true;
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    return false;
  }
}

export async function setJobEnabled(job: Job, enabled: boolean): Promise<void> {
  try {
    const updated = await api.jobs.update(job.id, { enabled });
    jobsStore.updateJob({ ...job, ...updated });
    showToast({
      message: enabled ? m.job_enabled_toast({ name: job.name }) : m.job_disabled_toast({ name: job.name }),
      variant: 'success',
    });
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
  }
}

export async function checkJobSpace(job: Job): Promise<void> {
  try {
    const r = await api.jobs.checkSpace(job.id);
    if (r.fits) {
      showToast({
        message: `${m.space_check_fits()}: ${m.space_check_transfer_size()} ${formatBytes(r.transfer_size)}, ${m.space_check_free_space()} ${formatBytes(r.destination_free)}`,
        variant: 'success',
      });
    } else {
      showToast({
        message: `${m.space_check_insufficient()}: ${m.space_check_deficit()} ${formatBytes(r.deficit ?? 0)}`,
        variant: 'error',
      });
    }
    jobsStore.refresh();
  } catch {
    showToast({ message: m.space_check_error(), variant: 'error' });
  }
}

/** Delete after confirmation; returns whether it was deleted. */
export async function deleteJob(job: Job): Promise<boolean> {
  const ok = await confirm({
    title: m.job_confirm_delete_title(),
    message: m.job_confirm_delete_message({ name: job.name }),
    confirmText: m.common_delete(),
    variant: 'danger',
  });
  if (!ok) return false;
  try {
    await api.jobs.delete(job.id);
    jobsStore.removeJob(job.id);
    showToast({ message: m.job_deleted_toast({ name: job.name }), variant: 'success' });
    return true;
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.job_error_delete(), variant: 'error' });
    return false;
  }
}

export function cloneJob(job: Job): void {
  push(`/jobs/new?clone=${job.id}`);
}

/** Open the log of the job's latest run (running or finished). */
export async function openLatestLog(job: Job): Promise<void> {
  const fromStore = get(jobsStore).runs.find((r) => r.job_id === job.id);
  if (fromStore) {
    showRunLog(fromStore.id, job.id);
    return;
  }
  try {
    const [latest] = await api.runs.list(job.id, 1, 0);
    if (latest) showRunLog(latest.id, job.id);
    else showToast({ message: m.job_no_runs_yet(), variant: 'info' });
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
  }
}

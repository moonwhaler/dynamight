import { api } from '$lib/api';
import type { Job } from '$lib/types';
import type { DraftSection, JobDraft } from '$lib/jobDraft.svelte';
import { jobsStore } from '$lib/stores/jobs';
import { showToast } from '../../ui/Toast.svelte';
import { confirm } from '../../ui/ConfirmDialog.svelte';
import * as m from '$lib/paraglide/messages.js';

export type SaveResult =
  | { ok: true; job: Job; schedulesFailed: number }
  | { ok: false; invalid?: DraftSection };

/**
 * Validate, confirm mirror mode when it is newly on, then create or update
 * the job and apply the schedule changes. Shows the toasts; the caller
 * decides where to go next.
 */
export async function saveDraft(draft: JobDraft, jobId: number | null, mirrorWasOn: boolean): Promise<SaveResult> {
  const invalid = draft.firstInvalid();
  if (invalid) {
    showToast({ message: invalid.message, variant: 'error' });
    return { ok: false, invalid: invalid.section };
  }

  if (draft.syncOptions.delete_extraneous && !mirrorWasOn) {
    const ok = await confirm({
      title: m.job_confirm_mirror_title(),
      message: m.job_confirm_mirror_message(),
      confirmText: m.job_confirm_mirror_button(),
      variant: 'danger',
    });
    if (!ok) return { ok: false };
  }

  try {
    const request = draft.toRequest();
    const job = jobId === null ? await api.jobs.create(request) : await api.jobs.update(jobId, request);
    const schedulesFailed = await draft.saveSchedules(job.id);
    draft.setSavedSchedules(await api.schedules.list(job.id).catch(() => draft.schedules));
    draft.markSaved();
    if (jobId === null) jobsStore.addJob(job);
    else jobsStore.updateJob(job);
    jobsStore.refresh();

    if (schedulesFailed > 0) {
      showToast({ message: m.job_schedules_save_failed({ count: schedulesFailed }), variant: 'error' });
    } else {
      showToast({ message: jobId === null ? m.job_toast_created() : m.job_toast_saved(), variant: 'success' });
    }
    return { ok: true, job, schedulesFailed };
  } catch (e) {
    showToast({ message: e instanceof Error ? e.message : m.job_error_save(), variant: 'error' });
    return { ok: false };
  }
}

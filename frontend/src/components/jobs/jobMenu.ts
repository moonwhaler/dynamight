import { push } from 'svelte-spa-router';
import type { Job } from '$lib/types';
import type { MenuItem } from '../ui/Menu.svelte';
import { checkJobSpace, cloneJob, deleteJob, isRunning, openLatestLog, setJobEnabled } from '$lib/jobActions';
import * as m from '$lib/paraglide/messages.js';

/** The overflow actions of a job, the same everywhere a job is shown */
export function jobMenuItems(job: Job, opts: { onDeleted?: () => void; withOpen?: boolean } = {}): MenuItem[] {
  const running = isRunning(job);
  const items: MenuItem[] = [];
  if (opts.withOpen !== false) items.push({ label: m.jobs_menu_open(), icon: 'eye', href: `#/jobs/${job.id}` });
  items.push(
    { label: m.jobs_menu_edit(), icon: 'edit', href: `#/jobs/${job.id}/settings` },
    { label: m.job_menu_latest_log(), icon: 'log', onclick: () => openLatestLog(job), disabled: !job.last_run_status },
    { label: m.space_check_button(), icon: 'gauge', onclick: () => checkJobSpace(job), disabled: running },
    { label: m.common_clone(), icon: 'copy', onclick: () => cloneJob(job) },
    'separator',
    job.enabled
      ? { label: m.job_menu_disable(), icon: 'pause', onclick: () => setJobEnabled(job, false) }
      : { label: m.job_menu_enable(), icon: 'power', onclick: () => setJobEnabled(job, true) },
    {
      label: m.common_delete(),
      icon: 'trash',
      danger: true,
      disabled: running,
      onclick: async () => {
        if (await deleteJob(job)) {
          opts.onDeleted?.();
          if (location.hash.startsWith(`#/jobs/${job.id}`)) push('/jobs');
        }
      },
    },
  );
  return items;
}

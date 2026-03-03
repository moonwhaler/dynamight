import * as m from '$lib/paraglide/messages.js';

export function formatStatus(status: string): string {
  switch (status) {
    case 'completed': return m.history_status_completed();
    case 'running':   return m.history_status_running();
    case 'failed':    return m.history_status_failed();
    case 'cancelled': return m.history_status_cancelled();
    case 'pending':   return m.history_status_pending();
    default:          return status;
  }
}

export function getStatusIndicator(status: string | null | undefined): { color: string; label: string } {
  switch (status) {
    case 'completed': return { color: 'bg-green-500', label: m.job_card_last_run_succeeded() };
    case 'failed':    return { color: 'bg-red-500',    label: m.job_card_last_run_failed() };
    case 'running':   return { color: 'bg-blue-500',   label: m.job_card_currently_running() };
    case 'cancelled': return { color: 'bg-orange-500', label: m.job_card_last_run_cancelled() };
    case 'pending':   return { color: 'bg-yellow-500', label: m.job_card_run_pending() };
    default:          return { color: '', label: '' };
  }
}

export function getStatusBadgeClass(status: string): string {
  switch (status) {
    case 'completed': return 'badge-success';
    case 'running':   return 'badge-info';
    case 'failed':    return 'badge-error';
    case 'cancelled': return 'badge-warning';
    default:          return 'badge-gray';
  }
}

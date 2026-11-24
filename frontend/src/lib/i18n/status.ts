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

export function getStatusBadgeClass(status: string): string {
  switch (status) {
    case 'completed': return 'badge-success';
    case 'running':   return 'badge-info';
    case 'failed':    return 'badge-error';
    case 'cancelled': return 'badge-warning';
    default:          return 'badge-gray';
  }
}

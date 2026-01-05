import * as m from '$lib/paraglide/messages.js';

/**
 * Format a job run status to a localized string.
 */
export function formatStatus(status: string): string {
  switch (status) {
    case 'completed':
      return m.history_status_completed();
    case 'running':
      return m.history_status_running();
    case 'failed':
      return m.history_status_failed();
    case 'cancelled':
      return m.history_status_cancelled();
    case 'pending':
      return m.history_status_pending();
    default:
      return status;
  }
}

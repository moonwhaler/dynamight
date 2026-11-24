import * as m from '$lib/paraglide/messages.js';

function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/**
 * A file's modification time for a list: "Today 09:14", "Yesterday 18:02",
 * "12 Nov" this year, "12 Nov 2025" before. `timestamp` is in Unix seconds.
 */
export function formatModified(timestamp: number | null, now = new Date(), locale?: string): string {
  if (timestamp === null || timestamp === undefined) return '—';
  const date = new Date(timestamp * 1000);
  const time = date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
  if (sameDay(date, now)) return m.filebrowser_date_today({ time });
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (sameDay(date, yesterday)) return m.filebrowser_date_yesterday_at({ time });
  return date.toLocaleDateString(locale, {
    day: 'numeric',
    month: 'short',
    ...(date.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' }),
  });
}

/** Full date and time, for a tooltip */
export function formatModifiedFull(timestamp: number | null, locale?: string): string {
  if (timestamp === null || timestamp === undefined) return '';
  return new Date(timestamp * 1000).toLocaleString(locale, { dateStyle: 'medium', timeStyle: 'short' });
}

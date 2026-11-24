import { writable } from 'svelte/store';
import { api } from '../api';
import { getLocale } from '$lib/paraglide/runtime.js';
import { describeCron } from '../utils/cron';

/**
 * The timezone the server interprets schedules in (`[server] timezone`).
 * "Daily at 02:00" means 02:00 there, which is not always the browser's zone.
 * Null until loaded (or if loading failed): times are then shown as before.
 */
export const serverTimezone = writable<string | null>(null);

let loading: Promise<void> | null = null;

/** Load the server timezone once; later calls reuse the first request. */
export function loadServerTimezone(): Promise<void> {
  if (!loading) {
    loading = api.system
      .info()
      .then((info) => serverTimezone.set(info.timezone))
      .catch(() => {
        // Retry on the next call; until then times show without a zone
        loading = null;
      });
  }
  return loading;
}

const UTC_ALIASES = new Set(['UTC', 'Etc/UTC', 'Etc/GMT', 'GMT', 'Etc/Universal', 'Universal', 'Zulu']);

function normalize(tz: string): string {
  return UTC_ALIASES.has(tz) ? 'UTC' : tz;
}

export function browserTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}

/**
 * The server zone if it differs from the browser's (so schedule times need a
 * zone next to them), otherwise null.
 */
export function differingTimezone(serverTz: string | null, browserTz = browserTimezone()): string | null {
  if (!serverTz || normalize(serverTz) === normalize(browserTz)) return null;
  return serverTz;
}

/**
 * " (Zone)" to put after a schedule description such as "Daily at 02:00"
 * when that clock time is in another zone than the browser's. Intervals
 * ("every 2 hours") run the same in every zone and get no suffix.
 */
export function scheduleZoneSuffix(cron: string, serverTz: string | null, browserTz = browserTimezone()): string {
  const zone = differingTimezone(serverTz, browserTz);
  if (!zone) return '';
  const kind = describeCron(cron).kind;
  return kind === 'every_minutes' || kind === 'every_hours' ? '' : ` (${zone})`;
}

/**
 * Format a time for a schedule: in the server's zone with the zone named
 * when it differs from the browser's, else in local time as usual.
 */
export function formatScheduleTime(
  dateStr: string,
  serverTz: string | null,
  browserTz = browserTimezone()
): string {
  const date = new Date(dateStr);
  const zone = differingTimezone(serverTz, browserTz);
  const options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' };
  if (!zone) return date.toLocaleString(getLocale(), options);
  try {
    return `${date.toLocaleString(getLocale(), { ...options, timeZone: zone })} (${zone})`;
  } catch {
    // A zone name the browser does not know: show local time instead
    return date.toLocaleString(getLocale(), options);
  }
}

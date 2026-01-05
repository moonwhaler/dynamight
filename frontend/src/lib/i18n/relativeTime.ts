import * as m from '$lib/paraglide/messages.js';
import { getLocale } from '$lib/paraglide/runtime.js';

/**
 * Format a date as a relative time string using translations.
 * Returns strings like "just now", "5m ago", "2h ago", "3d ago"
 */
export function formatRelativeTime(dateStr: string | null | undefined): string {
  if (!dateStr) return '';

  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);
  const diffWeeks = Math.floor(diffDays / 7);

  if (diffMins < 1) {
    return m.time_just_now();
  }
  if (diffMins < 60) {
    return m.time_minutes_ago({ count: diffMins });
  }
  if (diffHours < 24) {
    return m.time_hours_ago({ count: diffHours });
  }
  if (diffDays < 7) {
    return m.time_days_ago({ count: diffDays });
  }
  if (diffWeeks < 4) {
    return m.time_weeks_ago({ count: diffWeeks });
  }

  // For older dates, use locale-aware formatting
  return formatLocalizedDate(date);
}

/**
 * Format a date using the current locale
 */
export function formatLocalizedDate(date: Date): string {
  const locale = getLocale();
  return date.toLocaleDateString(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/**
 * Format a date and time using the current locale
 */
export function formatLocalizedDateTime(date: Date): string {
  const locale = getLocale();
  return date.toLocaleString(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * Format a duration in milliseconds to a human-readable string
 */
export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    const remainingMins = minutes % 60;
    return `${hours}h ${remainingMins}m`;
  }
  if (minutes > 0) {
    const remainingSecs = seconds % 60;
    return `${minutes}m ${remainingSecs}s`;
  }
  return `${seconds}s`;
}

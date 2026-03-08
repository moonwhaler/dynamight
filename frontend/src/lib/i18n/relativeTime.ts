import * as m from '$lib/paraglide/messages.js';
import { getLocale } from '$lib/paraglide/runtime.js';

export function formatRelativeTime(dateStr: string | null | undefined): string {
  if (!dateStr) return m.common_never();

  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);
  const diffWeeks = Math.floor(diffDays / 7);

  if (diffMins < 1) return m.time_just_now();
  if (diffMins < 60) return diffMins === 1 ? m.time_minute_ago() : m.time_minutes_ago({ count: diffMins });
  if (diffHours < 24) return diffHours === 1 ? m.time_hour_ago() : m.time_hours_ago({ count: diffHours });
  if (diffDays < 7) return diffDays === 1 ? m.time_day_ago() : m.time_days_ago({ count: diffDays });
  if (diffWeeks < 4) return diffWeeks === 1 ? m.time_week_ago() : m.time_weeks_ago({ count: diffWeeks });
  return formatLocalizedDate(date);
}

export function formatTimeUntil(dateStr: string | null | undefined): string {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  if (diffMs <= 0) return m.time_just_now();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);
  if (diffMins < 60) return diffMins === 1 ? m.time_in_minute() : m.time_in_minutes({ count: diffMins });
  if (diffHours < 24) return diffHours === 1 ? m.time_in_hour() : m.time_in_hours({ count: diffHours });
  if (diffDays < 7) return diffDays === 1 ? m.time_in_day() : m.time_in_days({ count: diffDays });
  return formatLocalizedDate(date);
}

export function formatLocalizedDate(date: Date): string {
  const locale = getLocale();
  return date.toLocaleDateString(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

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

export function formatDateTime(dateStr: string | null | undefined): string {
  if (!dateStr) return m.common_never();
  return formatLocalizedDateTime(new Date(dateStr));
}

export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

export function formatDurationBetween(start: string | null, end: string | null): string {
  if (!start) return '-';
  const startDate = new Date(start);
  const endDate = end ? new Date(end) : new Date();
  return formatDuration(endDate.getTime() - startDate.getTime());
}

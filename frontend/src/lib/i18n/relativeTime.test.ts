import { describe, it, expect } from 'vitest';
import { formatRelativeTime, formatTimeUntil, formatDuration, formatDurationBetween } from './relativeTime';

describe('formatRelativeTime', () => {
  it('returns never for null', () => {
    expect(formatRelativeTime(null)).toBe('common_never');
  });

  it('returns never for undefined', () => {
    expect(formatRelativeTime(undefined)).toBe('common_never');
  });

  it('returns just now for recent time', () => {
    const now = new Date().toISOString();
    expect(formatRelativeTime(now)).toBe('time_just_now');
  });

  it('returns minutes ago', () => {
    const fiveMinAgo = new Date(Date.now() - 5 * 60 * 1000).toISOString();
    expect(formatRelativeTime(fiveMinAgo)).toBe('time_minutes_ago(count=5)');
  });

  it('returns singular minute ago', () => {
    const oneMinAgo = new Date(Date.now() - 61 * 1000).toISOString();
    expect(formatRelativeTime(oneMinAgo)).toBe('time_minute_ago');
  });

  it('returns hours ago', () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 3600 * 1000).toISOString();
    expect(formatRelativeTime(twoHoursAgo)).toBe('time_hours_ago(count=2)');
  });

  it('returns singular hour ago', () => {
    const oneHourAgo = new Date(Date.now() - 3600 * 1000).toISOString();
    expect(formatRelativeTime(oneHourAgo)).toBe('time_hour_ago');
  });

  it('returns days ago', () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 86400 * 1000).toISOString();
    expect(formatRelativeTime(threeDaysAgo)).toBe('time_days_ago(count=3)');
  });

  it('returns weeks ago', () => {
    const twoWeeksAgo = new Date(Date.now() - 14 * 86400 * 1000).toISOString();
    expect(formatRelativeTime(twoWeeksAgo)).toBe('time_weeks_ago(count=2)');
  });

  it('returns formatted date for old dates', () => {
    const oldDate = new Date(Date.now() - 60 * 86400 * 1000).toISOString();
    const result = formatRelativeTime(oldDate);
    // Should be a localized date, not a relative time
    expect(result).not.toContain('ago');
  });
});

describe('formatTimeUntil', () => {
  it('returns empty for null', () => {
    expect(formatTimeUntil(null)).toBe('');
  });

  it('returns just now for past date', () => {
    const past = new Date(Date.now() - 60000).toISOString();
    expect(formatTimeUntil(past)).toBe('time_just_now');
  });

  it('returns in minutes', () => {
    const future = new Date(Date.now() + 5 * 60 * 1000 + 1000).toISOString();
    expect(formatTimeUntil(future)).toBe('time_in_minutes(count=5)');
  });

  it('returns in hours', () => {
    const future = new Date(Date.now() + 2 * 3600 * 1000 + 1000).toISOString();
    expect(formatTimeUntil(future)).toBe('time_in_hours(count=2)');
  });

  it('returns in days', () => {
    const future = new Date(Date.now() + 3 * 86400 * 1000 + 1000).toISOString();
    expect(formatTimeUntil(future)).toBe('time_in_days(count=3)');
  });
});

describe('formatDuration', () => {
  it('formats seconds', () => {
    expect(formatDuration(5000)).toBe('5s');
  });

  it('formats minutes and seconds', () => {
    expect(formatDuration(125000)).toBe('2m 5s');
  });

  it('formats hours and minutes', () => {
    expect(formatDuration(5400000)).toBe('1h 30m');
  });

  it('formats zero', () => {
    expect(formatDuration(0)).toBe('0s');
  });
});

describe('formatDurationBetween', () => {
  it('returns dash for null start', () => {
    expect(formatDurationBetween(null, null)).toBe('-');
  });

  it('calculates duration between dates', () => {
    const start = '2024-01-01T00:00:00Z';
    const end = '2024-01-01T01:30:00Z';
    expect(formatDurationBetween(start, end)).toBe('1h 30m');
  });

  it('uses now when end is null', () => {
    const recentStart = new Date(Date.now() - 60000).toISOString();
    const result = formatDurationBetween(recentStart, null);
    expect(result).toBe('1m 0s');
  });
});

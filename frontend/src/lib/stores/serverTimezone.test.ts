import { describe, it, expect, vi } from 'vitest';

vi.mock('$lib/paraglide/runtime.js', () => ({ getLocale: () => 'en-US' }));
vi.mock('../api', () => ({ api: { system: { info: vi.fn() } } }));

import { differingTimezone, formatScheduleTime, scheduleZoneSuffix } from './serverTimezone';

describe('differingTimezone', () => {
  it('is null when the zones match or the server zone is unknown', () => {
    expect(differingTimezone('Europe/Berlin', 'Europe/Berlin')).toBeNull();
    expect(differingTimezone('UTC', 'Etc/UTC')).toBeNull();
    expect(differingTimezone(null, 'Europe/Berlin')).toBeNull();
  });

  it('returns the server zone when it differs', () => {
    expect(differingTimezone('UTC', 'Europe/Berlin')).toBe('UTC');
  });
});

describe('formatScheduleTime', () => {
  const at = '2026-11-25T02:00:00Z';

  it('shows the server time with its zone when the zones differ', () => {
    const text = formatScheduleTime(at, 'UTC', 'Europe/Berlin');
    expect(text).toContain('2:00');
    expect(text).toContain('(UTC)');
  });

  it('shows no zone when they match', () => {
    expect(formatScheduleTime(at, 'Europe/Berlin', 'Europe/Berlin')).not.toContain('(');
  });

  it('falls back to local time for a zone the browser does not know', () => {
    expect(formatScheduleTime(at, 'Mars/Olympus', 'Europe/Berlin')).not.toContain('Mars');
  });
});

describe('scheduleZoneSuffix', () => {
  it('names the zone for clock times only', () => {
    expect(scheduleZoneSuffix('0 2 * * *', 'UTC', 'Europe/Berlin')).toBe(' (UTC)');
    expect(scheduleZoneSuffix('0 */2 * * *', 'UTC', 'Europe/Berlin')).toBe('');
    expect(scheduleZoneSuffix('0 2 * * *', 'Europe/Berlin', 'Europe/Berlin')).toBe('');
  });
});

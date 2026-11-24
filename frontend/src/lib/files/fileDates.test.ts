import { describe, it, expect } from 'vitest';
import { formatModified } from './fileDates';

const now = new Date(2026, 10, 24, 15, 0, 0);
const ts = (d: Date) => Math.floor(d.getTime() / 1000);

describe('formatModified', () => {
  it('says today and yesterday with the time', () => {
    expect(formatModified(ts(new Date(2026, 10, 24, 9, 14)), now, 'en-GB')).toBe('filebrowser_date_today(time=09:14)');
    expect(formatModified(ts(new Date(2026, 10, 23, 18, 2)), now, 'en-GB')).toBe('filebrowser_date_yesterday_at(time=18:02)');
  });

  it('leaves the year out for this year only', () => {
    expect(formatModified(ts(new Date(2026, 1, 3)), now, 'en-GB')).toBe('3 Feb');
    expect(formatModified(ts(new Date(2025, 10, 12)), now, 'en-GB')).toBe('12 Nov 2025');
  });

  it('shows a dash without a date', () => {
    expect(formatModified(null, now)).toBe('—');
  });
});

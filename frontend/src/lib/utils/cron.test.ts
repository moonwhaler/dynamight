import { describe, it, expect } from 'vitest';
import { describeCron } from './cron';

describe('describeCron', () => {
  it('reads the patterns the schedule picker creates', () => {
    expect(describeCron('30 2 * * *')).toEqual({ kind: 'daily', time: '02:30' });
    expect(describeCron('0 3 * * 1,3,5')).toEqual({ kind: 'days', days: [1, 3, 5], time: '03:00' });
    expect(describeCron('0 4 15 * *')).toEqual({ kind: 'monthly', day: 15, time: '04:00' });
    expect(describeCron('*/15 * * * *')).toEqual({ kind: 'every_minutes', count: 15 });
    expect(describeCron('0 */6 * * *')).toEqual({ kind: 'every_hours', count: 6 });
  });

  it('expands day ranges and treats 7 as Sunday', () => {
    expect(describeCron('0 1 * * 1-5')).toEqual({ kind: 'days', days: [1, 2, 3, 4, 5], time: '01:00' });
    expect(describeCron('0 1 * * 6,7')).toEqual({ kind: 'days', days: [0, 6], time: '01:00' });
    expect(describeCron('0 1 * * 0-6')).toEqual({ kind: 'daily', time: '01:00' });
  });

  it('does not summarize what it cannot describe exactly', () => {
    for (const expr of [
      '0 9-17 * * *', // hour range
      '5 * * * *', // hourly at :05
      '*/15 9 * * *', // minute step within one hour
      '0 */2 * * 1-5', // interval limited to weekdays
      '0 0 1 1 *', // month restriction
      '0 3 * * MON', // day name
      '0 3 1,15 * *', // several days of month
      '0 3 1 * 1', // day of month and weekday
      '0 25 * * *', // out of range
      '* */2 * * *', // every minute of every 2nd hour
      'bogus',
    ]) {
      expect(describeCron(expr), expr).toEqual({ kind: 'custom' });
    }
  });
});

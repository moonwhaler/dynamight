/**
 * Structured reading of a standard 5-field cron expression, for display.
 *
 * Only patterns that can be described exactly are recognized; anything else
 * (ranges in the time fields, steps with offsets, month restrictions, names
 * like MON) is `custom`, so the caller shows the raw expression instead of a
 * wrong summary.
 */
export type CronDescription =
  | { kind: 'every_minutes'; count: number }
  | { kind: 'every_hours'; count: number }
  | { kind: 'daily'; time: string }
  /** Days of the week, 0 = Sunday, sorted and without duplicates */
  | { kind: 'days'; days: number[]; time: string }
  | { kind: 'monthly'; day: number; time: string }
  | { kind: 'custom' };

function num(field: string, min: number, max: number): number | null {
  if (!/^\d+$/.test(field)) return null;
  const n = parseInt(field, 10);
  return n >= min && n <= max ? n : null;
}

function step(field: string): number | null {
  const match = /^\*\/(\d+)$/.exec(field);
  if (!match) return null;
  const n = parseInt(match[1], 10);
  return n >= 1 ? n : null;
}

/** Day-of-week field as a list of days (0-6), or null if it is not a plain list or range. */
function weekdays(field: string): number[] | null {
  const days = new Set<number>();
  for (const part of field.split(',')) {
    const range = /^(\d+)-(\d+)$/.exec(part);
    if (range) {
      const from = num(range[1], 0, 7);
      const to = num(range[2], 0, 7);
      if (from === null || to === null || from > to) return null;
      for (let d = from; d <= to; d++) days.add(d % 7);
    } else {
      const d = num(part, 0, 7);
      if (d === null) return null;
      days.add(d % 7);
    }
  }
  return [...days].sort((a, b) => a - b);
}

export function describeCron(expr: string): CronDescription {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 5) return { kind: 'custom' };
  const [minute, hour, dom, month, dow] = parts;
  if (month !== '*') return { kind: 'custom' };

  const minuteStep = step(minute);
  if (minuteStep !== null && hour === '*' && dom === '*' && dow === '*') {
    return { kind: 'every_minutes', count: minuteStep };
  }
  const hourStep = step(hour);
  if (minute === '0' && hourStep !== null && dom === '*' && dow === '*') {
    return { kind: 'every_hours', count: hourStep };
  }

  const m = num(minute, 0, 59);
  const h = num(hour, 0, 23);
  if (m === null || h === null) return { kind: 'custom' };
  const time = `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`;

  if (dom === '*' && dow === '*') return { kind: 'daily', time };
  if (dom === '*') {
    const days = weekdays(dow);
    if (!days) return { kind: 'custom' };
    if (days.length === 7) return { kind: 'daily', time };
    return { kind: 'days', days, time };
  }
  if (dow === '*') {
    const day = num(dom, 1, 31);
    if (day !== null) return { kind: 'monthly', day, time };
  }
  return { kind: 'custom' };
}

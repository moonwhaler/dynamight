import { describeCron } from '$lib/utils/cron';
import * as m from '$lib/paraglide/messages.js';

export function dayName(index: number): string {
  return [m.day_sun(), m.day_mon(), m.day_tue(), m.day_wed(), m.day_thu(), m.day_fri(), m.day_sat()][index];
}

/** "Daily at 02:00", "Mon, Wed at 18:00" … or the raw expression */
export function formatCron(cron: string): string {
  const d = describeCron(cron);
  switch (d.kind) {
    case 'every_minutes':
      return m.schedule_every_minutes({ count: d.count });
    case 'every_hours':
      return m.schedule_every_hours({ count: d.count });
    case 'daily':
      return m.schedule_daily_at({ time: d.time });
    case 'days':
      return m.schedule_days_at({ days: d.days.map(dayName).join(', '), time: d.time });
    case 'monthly':
      return m.schedule_monthly_on({ day: d.day, time: d.time });
    default:
      return cron;
  }
}

import * as m from '$lib/paraglide/messages.js';
import type { IconName } from '../ui/Icon.svelte';

export interface NavItem {
  path: string;
  label: () => string;
  icon: IconName;
}

export const NAV_ITEMS: NavItem[] = [
  { path: '/', label: m.nav_overview, icon: 'home' },
  { path: '/jobs', label: m.nav_jobs, icon: 'jobs' },
  { path: '/activity', label: m.nav_activity, icon: 'clock' },
  { path: '/files', label: m.nav_files, icon: 'folder' },
  { path: '/settings', label: m.common_settings, icon: 'settings' },
];

export function isActive(path: string, current: string): boolean {
  if (path === '/') return current === '/';
  return current === path || current.startsWith(path + '/');
}

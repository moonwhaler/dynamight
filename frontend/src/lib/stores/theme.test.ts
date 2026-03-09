import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('themeStore', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
    vi.resetModules();
  });

  it('defaults to light theme', async () => {
    const { themeStore } = await import('./theme');
    let value: string = '';
    const unsub = themeStore.subscribe((v) => { value = v; });
    expect(value).toBe('light');
    unsub();
  });

  it('reads stored theme from localStorage', async () => {
    localStorage.setItem('dynamight-theme', 'dark');
    const { themeStore } = await import('./theme');
    let value: string = '';
    const unsub = themeStore.subscribe((v) => { value = v; });
    expect(value).toBe('dark');
    unsub();
  });

  it('toggles from light to dark', async () => {
    const { themeStore } = await import('./theme');
    let value: string = '';
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.toggle();
    expect(value).toBe('dark');
    expect(localStorage.getItem('dynamight-theme')).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    unsub();
  });

  it('toggles from dark to light', async () => {
    localStorage.setItem('dynamight-theme', 'dark');
    const { themeStore } = await import('./theme');
    let value: string = '';
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.toggle();
    expect(value).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    unsub();
  });
});

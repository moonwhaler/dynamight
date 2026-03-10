import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('themeStore', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
    document.documentElement.removeAttribute('style');
    vi.resetModules();
  });

  it('defaults to light mode with paper base and ocean accent', async () => {
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });
    expect(value.mode).toBe('light');
    expect(value.base).toBe('paper');
    expect(value.accent).toBe('ocean');
    unsub();
  });

  it('reads stored theme config from localStorage', async () => {
    localStorage.setItem('dynamight-theme', JSON.stringify({ mode: 'dark', base: 'slate', accent: 'violet' }));
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });
    expect(value.mode).toBe('dark');
    expect(value.base).toBe('slate');
    expect(value.accent).toBe('violet');
    unsub();
  });

  it('handles legacy string format', async () => {
    localStorage.setItem('dynamight-theme', '"dark"');
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });
    expect(value.mode).toBe('dark');
    expect(value.base).toBe('paper');
    expect(value.accent).toBe('ocean');
    unsub();
  });

  it('toggles from light to dark', async () => {
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.toggle();
    expect(value.mode).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    unsub();
  });

  it('toggles from dark to light', async () => {
    localStorage.setItem('dynamight-theme', JSON.stringify({ mode: 'dark', base: 'paper', accent: 'ocean' }));
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.toggle();
    expect(value.mode).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    unsub();
  });

  it('sets base theme', async () => {
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.setBase('slate');
    expect(value.base).toBe('slate');

    const stored = JSON.parse(localStorage.getItem('dynamight-theme')!);
    expect(stored.base).toBe('slate');
    unsub();
  });

  it('sets accent color', async () => {
    const { themeStore } = await import('./theme');
    let value: any;
    const unsub = themeStore.subscribe((v) => { value = v; });

    themeStore.setAccent('rose');
    expect(value.accent).toBe('rose');

    const stored = JSON.parse(localStorage.getItem('dynamight-theme')!);
    expect(stored.accent).toBe('rose');
    unsub();
  });

  it('applies CSS variables to document', async () => {
    const { themeStore } = await import('./theme');
    const unsub = themeStore.subscribe(() => {});

    const style = document.documentElement.getAttribute('style');
    expect(style).toContain('--c-gray-50');
    expect(style).toContain('--c-primary-600');
    expect(style).toContain('--c-surface');
    unsub();
  });

  it('persists CSS vars to localStorage for flash prevention', async () => {
    const { themeStore } = await import('./theme');
    const unsub = themeStore.subscribe(() => {});

    const vars = localStorage.getItem('dynamight-theme-vars');
    expect(vars).toContain('--c-gray-50');
    unsub();
  });
});

import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('preferencesStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it('has correct defaults', async () => {
    const { preferencesStore } = await import('./preferences');
    let prefs: Record<string, unknown> = {};
    const unsub = preferencesStore.subscribe((v) => { prefs = v as unknown as Record<string, unknown>; });

    expect(prefs.showLogViewerAfterManualRun).toBe(true);
    expect(prefs.autoShowLastPage).toBe(true);
    expect(prefs.logRefreshInterval).toBe(2);
    expect(prefs.confirmKillProcess).toBe(true);
    unsub();
  });

  it('persists changes to localStorage', async () => {
    const { preferencesStore } = await import('./preferences');
    preferencesStore.setShowLogViewerAfterManualRun(false);

    const stored = JSON.parse(localStorage.getItem('dynamight-preferences')!);
    expect(stored.showLogViewerAfterManualRun).toBe(false);
  });

  it('loads saved preferences', async () => {
    localStorage.setItem('dynamight-preferences', JSON.stringify({
      showLogViewerAfterManualRun: false,
      logRefreshInterval: 10,
    }));

    const { preferencesStore } = await import('./preferences');
    let prefs: Record<string, unknown> = {};
    const unsub = preferencesStore.subscribe((v) => { prefs = v as unknown as Record<string, unknown>; });

    expect(prefs.showLogViewerAfterManualRun).toBe(false);
    expect(prefs.logRefreshInterval).toBe(10);
    // Defaults should still be applied for missing keys
    expect(prefs.confirmKillProcess).toBe(true);
    unsub();
  });

  it('handles corrupt localStorage gracefully', async () => {
    localStorage.setItem('dynamight-preferences', 'not-json');

    const { preferencesStore } = await import('./preferences');
    let prefs: Record<string, unknown> = {};
    const unsub = preferencesStore.subscribe((v) => { prefs = v as unknown as Record<string, unknown>; });

    expect(prefs.showLogViewerAfterManualRun).toBe(true);
    unsub();
  });

  it('updates individual preferences', async () => {
    const { preferencesStore } = await import('./preferences');
    let prefs: Record<string, unknown> = {};
    const unsub = preferencesStore.subscribe((v) => { prefs = v as unknown as Record<string, unknown>; });

    preferencesStore.setLogRefreshInterval(5);
    expect(prefs.logRefreshInterval).toBe(5);

    preferencesStore.setConfirmKillProcess(false);
    expect(prefs.confirmKillProcess).toBe(false);

    preferencesStore.setAutoShowLastPage(false);
    expect(prefs.autoShowLastPage).toBe(false);
    unsub();
  });
});

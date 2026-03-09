import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('createTablePreferencesStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it('has default visible columns', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-prefs',
      allColumns: ['a', 'b', 'c', 'd'] as const,
      fixedColumns: ['a', 'd'] as const,
      defaultVisible: ['a', 'b', 'd'] as const,
      defaultWidths: { a: 100, b: 200, c: 150, d: 80 },
    });

    let prefs = { visibleColumns: [] as string[], columnWidths: {} as Record<string, number> };
    const unsub = store.subscribe((v) => { prefs = v; });
    expect(prefs.visibleColumns).toEqual(['a', 'b', 'd']);
    unsub();
  });

  it('ignores visibility toggle on fixed columns', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-fixed',
      allColumns: ['a', 'b', 'd'] as const,
      fixedColumns: ['a', 'd'] as const,
      defaultVisible: ['a', 'b', 'd'] as const,
      defaultWidths: { a: 100, b: 200, d: 80 },
    });

    let prefs = { visibleColumns: [] as string[], columnWidths: {} as Record<string, number> };
    const unsub = store.subscribe((v) => { prefs = v; });

    store.setColumnVisibility('a', false);
    expect(prefs.visibleColumns).toContain('a');
    unsub();
  });

  it('adds and removes columns', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-toggle',
      allColumns: ['a', 'b', 'c', 'd'] as const,
      fixedColumns: ['a', 'd'] as const,
      defaultVisible: ['a', 'b', 'd'] as const,
      defaultWidths: { a: 100, b: 200, c: 150, d: 80 },
    });

    let prefs = { visibleColumns: [] as string[], columnWidths: {} as Record<string, number> };
    const unsub = store.subscribe((v) => { prefs = v; });

    // Add column c
    store.setColumnVisibility('c', true);
    expect(prefs.visibleColumns).toContain('c');
    // d should still be last
    expect(prefs.visibleColumns[prefs.visibleColumns.length - 1]).toBe('d');

    // Remove column b
    store.setColumnVisibility('b', false);
    expect(prefs.visibleColumns).not.toContain('b');
    unsub();
  });

  it('clamps column width minimum', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-width',
      allColumns: ['a', 'b'] as const,
      fixedColumns: ['a'] as const,
      defaultVisible: ['a', 'b'] as const,
      defaultWidths: { a: 100, b: 200 },
    });

    let prefs = { visibleColumns: [] as string[], columnWidths: {} as Record<string, number> };
    const unsub = store.subscribe((v) => { prefs = v; });

    store.setColumnWidth('b', 30);
    expect(prefs.columnWidths.b).toBe(60); // clamped to min 60
    unsub();
  });

  it('resets to defaults', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-reset',
      allColumns: ['a', 'b', 'c', 'd'] as const,
      fixedColumns: ['a', 'd'] as const,
      defaultVisible: ['a', 'b', 'd'] as const,
      defaultWidths: { a: 100, b: 200, c: 150, d: 80 },
    });

    store.setColumnVisibility('c', true);
    store.setColumnWidth('a', 500);

    let prefs = { visibleColumns: [] as string[], columnWidths: {} as Record<string, number> };
    const unsub = store.subscribe((v) => { prefs = v; });

    store.reset();
    expect(prefs.visibleColumns).toEqual(['a', 'b', 'd']);
    expect(prefs.columnWidths.a).toBe(100);
    unsub();
  });

  it('persists to localStorage', async () => {
    const { createTablePreferencesStore } = await import('./tablePreferences');
    const store = createTablePreferencesStore({
      storageKey: 'test-persist',
      allColumns: ['a', 'b', 'c'] as const,
      fixedColumns: ['a', 'c'] as const,
      defaultVisible: ['a', 'b', 'c'] as const,
      defaultWidths: { a: 100, b: 200, c: 80 },
    });

    store.setColumnWidth('b', 300);
    const stored = JSON.parse(localStorage.getItem('test-persist')!);
    expect(stored.columnWidths.b).toBe(300);
  });
});

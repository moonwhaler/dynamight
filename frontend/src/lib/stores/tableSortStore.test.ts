import { describe, it, expect, beforeEach, vi } from 'vitest';

describe('createTableSortStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it('has default values', async () => {
    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort',
      defaultSortBy: 'name' as const,
      defaultSortOrder: 'asc',
    });

    let state = { sortBy: '', sortOrder: '' };
    const unsub = store.subscribe((s) => { state = s as unknown as typeof state; });
    expect(state.sortBy).toBe('name');
    expect(state.sortOrder).toBe('asc');
    unsub();
  });

  it('toggles sort direction on same column', async () => {
    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort-toggle',
      defaultSortBy: 'name' as const,
    });

    let state = { sortBy: '', sortOrder: '' };
    const unsub = store.subscribe((s) => { state = s as unknown as typeof state; });

    store.handleSort('name');
    expect(state.sortOrder).toBe('desc');

    store.handleSort('name');
    expect(state.sortOrder).toBe('asc');
    unsub();
  });

  it('resets to asc on different column', async () => {
    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort-switch',
      defaultSortBy: 'name' as const,
      defaultSortOrder: 'desc',
    });

    let state = { sortBy: '', sortOrder: '' };
    const unsub = store.subscribe((s) => { state = s as unknown as typeof state; });

    store.handleSort('status');
    expect(state.sortBy).toBe('status');
    expect(state.sortOrder).toBe('asc');
    unsub();
  });

  it('persists to localStorage', async () => {
    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort-persist',
      defaultSortBy: 'name' as const,
    });

    store.setSortBy('date');
    const stored = JSON.parse(localStorage.getItem('test-sort-persist')!);
    expect(stored.sortBy).toBe('date');
  });

  it('loads from localStorage', async () => {
    localStorage.setItem('test-sort-load', JSON.stringify({ sortBy: 'size', sortOrder: 'desc' }));

    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort-load',
      defaultSortBy: 'name' as const,
    });

    let state = { sortBy: '', sortOrder: '' };
    const unsub = store.subscribe((s) => { state = s as unknown as typeof state; });
    expect(state.sortBy).toBe('size');
    expect(state.sortOrder).toBe('desc');
    unsub();
  });

  it('setSortOrder works', async () => {
    const { createTableSortStore } = await import('./tableSortStore');
    const store = createTableSortStore({
      storageKey: 'test-sort-order',
      defaultSortBy: 'name' as const,
    });

    let state = { sortBy: '', sortOrder: '' };
    const unsub = store.subscribe((s) => { state = s as unknown as typeof state; });

    store.setSortOrder('desc');
    expect(state.sortOrder).toBe('desc');
    unsub();
  });
});

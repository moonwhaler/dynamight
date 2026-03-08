import { writable } from 'svelte/store';

export type SortOrder = 'asc' | 'desc';

export interface SortState<T extends string> {
  sortBy: T;
  sortOrder: SortOrder;
}

export function createTableSortStore<T extends string>(config: {
  storageKey: string;
  defaultSortBy: T;
  defaultSortOrder?: SortOrder;
}) {
  const { storageKey, defaultSortBy, defaultSortOrder = 'asc' } = config;

  function loadFromStorage(): SortState<T> {
    if (typeof window === 'undefined') return { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
    try {
      const stored = localStorage.getItem(storageKey);
      if (!stored) return { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
      const parsed = JSON.parse(stored);
      if (parsed && typeof parsed.sortBy === 'string' && (parsed.sortOrder === 'asc' || parsed.sortOrder === 'desc')) {
        return { sortBy: parsed.sortBy as T, sortOrder: parsed.sortOrder };
      }
    } catch {
      // ignore
    }
    return { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
  }

  function saveToStorage(state: SortState<T>) {
    if (typeof window === 'undefined') return;
    try {
      localStorage.setItem(storageKey, JSON.stringify(state));
    } catch {
      // ignore
    }
  }

  const { subscribe, set } = writable<SortState<T>>(loadFromStorage());

  function persist(state: SortState<T>) {
    set(state);
    saveToStorage(state);
  }

  return {
    subscribe,

    handleSort(col: T) {
      let current: SortState<T> = loadFromStorage();
      // Re-read from store via subscription is complex; use a closure trick
      // We'll read from the store's current value by subscribing briefly
      let state: SortState<T> = { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
      const unsub = subscribe((s) => { state = s; });
      unsub();

      if (state.sortBy === col) {
        persist({ sortBy: col, sortOrder: state.sortOrder === 'asc' ? 'desc' : 'asc' });
      } else {
        persist({ sortBy: col, sortOrder: 'asc' });
      }
    },

    setSortBy(col: T) {
      let state: SortState<T> = { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
      const unsub = subscribe((s) => { state = s; });
      unsub();
      persist({ ...state, sortBy: col });
    },

    setSortOrder(order: SortOrder) {
      let state: SortState<T> = { sortBy: defaultSortBy, sortOrder: defaultSortOrder };
      const unsub = subscribe((s) => { state = s; });
      unsub();
      persist({ ...state, sortOrder: order });
    },
  };
}

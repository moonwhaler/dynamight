import { writable } from 'svelte/store';
import { createTableSortStore } from './tableSortStore';
export type { SortOrder } from './tableSortStore';

export type ColumnKey = 'job' | 'status' | 'sources' | 'destination' | 'last_run' | 'schedule' | 'options' | 'actions';

export const FIXED_COLUMNS: ColumnKey[] = ['job', 'actions'];
export const ALL_COLUMNS: ColumnKey[] = ['job', 'status', 'sources', 'destination', 'last_run', 'schedule', 'options', 'actions'];

const DEFAULT_VISIBLE: ColumnKey[] = ['job', 'status', 'sources', 'destination', 'last_run', 'options', 'actions'];
const DEFAULT_WIDTHS: Record<ColumnKey, number> = {
  job: 200,
  status: 90,
  sources: 90,
  destination: 200,
  last_run: 150,
  schedule: 180,
  options: 140,
  actions: 80,
};

interface TablePreferences<T extends string> {
  visibleColumns: T[];
  columnWidths: Record<T, number>;
}

export function createTablePreferencesStore<T extends string>(config: {
  storageKey: string;
  allColumns: readonly T[];
  fixedColumns: readonly T[];
  defaultVisible: readonly T[];
  defaultWidths: Record<T, number>;
}) {
  const { storageKey, allColumns, fixedColumns, defaultVisible, defaultWidths } = config;
  const firstFixed = fixedColumns[0];
  const lastFixed = fixedColumns[fixedColumns.length - 1];

  function validatePreferences(raw: unknown): TablePreferences<T> {
    const defaults: TablePreferences<T> = {
      visibleColumns: [...defaultVisible],
      columnWidths: { ...defaultWidths },
    };

    if (!raw || typeof raw !== 'object') return defaults;
    const r = raw as Record<string, unknown>;

    let visibleColumns: T[] = [...defaultVisible];
    if (Array.isArray(r.visibleColumns)) {
      const filtered = (r.visibleColumns as unknown[]).filter(
        (c): c is T => typeof c === 'string' && (allColumns as readonly string[]).includes(c)
      );
      const withoutFixed = filtered.filter((c) => !fixedColumns.includes(c));
      visibleColumns = [firstFixed, ...withoutFixed, lastFixed];
    }

    let columnWidths: Record<T, number> = { ...defaultWidths };
    if (r.columnWidths && typeof r.columnWidths === 'object') {
      const rawWidths = r.columnWidths as Record<string, unknown>;
      for (const col of allColumns) {
        const w = rawWidths[col as string];
        if (typeof w === 'number' && w >= 60) {
          columnWidths[col] = w;
        }
      }
    }

    return { visibleColumns, columnWidths };
  }

  function loadFromStorage(): TablePreferences<T> {
    if (typeof window === 'undefined') {
      return { visibleColumns: [...defaultVisible], columnWidths: { ...defaultWidths } };
    }
    try {
      const stored = localStorage.getItem(storageKey);
      if (!stored) return { visibleColumns: [...defaultVisible], columnWidths: { ...defaultWidths } };
      return validatePreferences(JSON.parse(stored));
    } catch {
      return { visibleColumns: [...defaultVisible], columnWidths: { ...defaultWidths } };
    }
  }

  function saveToStorage(prefs: TablePreferences<T>) {
    if (typeof window === 'undefined') return;
    try {
      localStorage.setItem(storageKey, JSON.stringify(prefs));
    } catch {
      // ignore storage errors
    }
  }

  const { subscribe, set, update } = writable<TablePreferences<T>>(loadFromStorage());

  function persist(prefs: TablePreferences<T>) {
    set(prefs);
    saveToStorage(prefs);
  }

  return {
    subscribe,

    setColumnVisibility(col: T, visible: boolean) {
      if (fixedColumns.includes(col)) return;
      update((prefs) => {
        const current = prefs.visibleColumns;
        let next: T[];
        if (visible) {
          if (current.includes(col)) return prefs;
          const withoutLast = current.filter((c) => c !== lastFixed);
          next = [...withoutLast, col, lastFixed];
        } else {
          next = current.filter((c) => c !== col);
        }
        const updated = { ...prefs, visibleColumns: next };
        saveToStorage(updated);
        return updated;
      });
    },

    setColumnOrder(cols: T[]) {
      update((prefs) => {
        const withoutFixed = cols.filter((c) => !fixedColumns.includes(c));
        const ordered: T[] = [firstFixed, ...withoutFixed, lastFixed];
        const updated = { ...prefs, visibleColumns: ordered };
        saveToStorage(updated);
        return updated;
      });
    },

    setColumnWidth(col: T, width: number) {
      update((prefs) => {
        const clamped = Math.max(60, Math.round(width));
        const updated = {
          ...prefs,
          columnWidths: { ...prefs.columnWidths, [col]: clamped },
        };
        saveToStorage(updated);
        return updated;
      });
    },

    reset() {
      persist({ visibleColumns: [...defaultVisible], columnWidths: { ...defaultWidths } });
    },
  };
}

export const tablePreferencesStore = createTablePreferencesStore<ColumnKey>({
  storageKey: 'dynamight-job-table-prefs',
  allColumns: ALL_COLUMNS,
  fixedColumns: FIXED_COLUMNS,
  defaultVisible: DEFAULT_VISIBLE,
  defaultWidths: DEFAULT_WIDTHS,
});
export { DEFAULT_VISIBLE, DEFAULT_WIDTHS };

export type JobsSortColumn = 'job' | 'status' | 'sources' | 'destination' | 'last_run';
export const jobsSortStore = createTableSortStore<JobsSortColumn>({
  storageKey: 'dynamight-job-sort',
  defaultSortBy: 'job',
  defaultSortOrder: 'asc',
});

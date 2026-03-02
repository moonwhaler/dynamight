import { writable } from 'svelte/store';

export type ColumnKey = 'job' | 'status' | 'sources' | 'destination' | 'last_run' | 'schedule' | 'options' | 'actions';

const STORAGE_KEY = 'dynamight-job-table-prefs';

export const FIXED_COLUMNS: ColumnKey[] = ['job', 'actions'];
export const ALL_COLUMNS: ColumnKey[] = ['job', 'status', 'sources', 'destination', 'last_run', 'schedule', 'options', 'actions'];

const DEFAULT_VISIBLE: ColumnKey[] = ['job', 'status', 'sources', 'destination', 'last_run', 'options', 'actions'];
const DEFAULT_WIDTHS: Record<ColumnKey, number> = {
  job: 200,
  status: 90,
  sources: 90,
  destination: 200,
  last_run: 120,
  schedule: 180,
  options: 140,
  actions: 80,
};

interface TablePreferences {
  visibleColumns: ColumnKey[];
  columnWidths: Record<ColumnKey, number>;
}

function validatePreferences(raw: unknown): TablePreferences {
  const defaults: TablePreferences = {
    visibleColumns: DEFAULT_VISIBLE,
    columnWidths: { ...DEFAULT_WIDTHS },
  };

  if (!raw || typeof raw !== 'object') return defaults;
  const r = raw as Record<string, unknown>;

  // Validate visibleColumns
  let visibleColumns: ColumnKey[] = DEFAULT_VISIBLE;
  if (Array.isArray(r.visibleColumns)) {
    const filtered = r.visibleColumns.filter((c): c is ColumnKey =>
      ALL_COLUMNS.includes(c as ColumnKey)
    );
    // Ensure 'job' is first and 'actions' is last
    const withoutFixed = filtered.filter((c) => !FIXED_COLUMNS.includes(c));
    visibleColumns = ['job', ...withoutFixed, 'actions'];
  }

  // Validate columnWidths
  let columnWidths: Record<ColumnKey, number> = { ...DEFAULT_WIDTHS };
  if (r.columnWidths && typeof r.columnWidths === 'object') {
    const rawWidths = r.columnWidths as Record<string, unknown>;
    for (const col of ALL_COLUMNS) {
      const w = rawWidths[col];
      if (typeof w === 'number' && w >= 60) {
        columnWidths[col] = w;
      }
    }
  }

  return { visibleColumns, columnWidths };
}

function loadFromStorage(): TablePreferences {
  if (typeof window === 'undefined') {
    return { visibleColumns: DEFAULT_VISIBLE, columnWidths: { ...DEFAULT_WIDTHS } };
  }
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return { visibleColumns: DEFAULT_VISIBLE, columnWidths: { ...DEFAULT_WIDTHS } };
    return validatePreferences(JSON.parse(stored));
  } catch {
    return { visibleColumns: DEFAULT_VISIBLE, columnWidths: { ...DEFAULT_WIDTHS } };
  }
}

function saveToStorage(prefs: TablePreferences) {
  if (typeof window === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // ignore storage errors
  }
}

function createTablePreferencesStore() {
  const { subscribe, set, update } = writable<TablePreferences>(loadFromStorage());

  function persist(prefs: TablePreferences) {
    set(prefs);
    saveToStorage(prefs);
  }

  return {
    subscribe,

    setColumnVisibility(col: ColumnKey, visible: boolean) {
      if (FIXED_COLUMNS.includes(col)) return;
      update((prefs) => {
        const current = prefs.visibleColumns;
        let next: ColumnKey[];
        if (visible) {
          if (current.includes(col)) return prefs;
          // Insert before 'actions'
          const withoutActions = current.filter((c) => c !== 'actions');
          next = [...withoutActions, col, 'actions'];
        } else {
          next = current.filter((c) => c !== col);
        }
        const updated = { ...prefs, visibleColumns: next };
        saveToStorage(updated);
        return updated;
      });
    },

    setColumnOrder(cols: ColumnKey[]) {
      update((prefs) => {
        // Ensure fixed columns are in correct positions
        const withoutFixed = cols.filter((c) => !FIXED_COLUMNS.includes(c));
        const ordered: ColumnKey[] = ['job', ...withoutFixed, 'actions'];
        const updated = { ...prefs, visibleColumns: ordered };
        saveToStorage(updated);
        return updated;
      });
    },

    setColumnWidth(col: ColumnKey, width: number) {
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
      persist({ visibleColumns: DEFAULT_VISIBLE, columnWidths: { ...DEFAULT_WIDTHS } });
    },
  };
}

export const tablePreferencesStore = createTablePreferencesStore();
export { DEFAULT_VISIBLE, DEFAULT_WIDTHS };

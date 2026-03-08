import { createTablePreferencesStore } from './tablePreferences';

export type HistoryColumnKey = 'job' | 'status' | 'started' | 'duration' | 'files' | 'size' | 'actions';

export const HISTORY_FIXED: HistoryColumnKey[] = ['job', 'actions'];
export const HISTORY_ALL: HistoryColumnKey[] = ['job', 'status', 'started', 'duration', 'files', 'size', 'actions'];
export const HISTORY_DEFAULT_VISIBLE: HistoryColumnKey[] = HISTORY_ALL;
export const HISTORY_DEFAULT_WIDTHS: Record<HistoryColumnKey, number> = {
  job: 200,
  status: 100,
  started: 180,
  duration: 120,
  files: 100,
  size: 100,
  actions: 90,
};

export const historyTablePreferencesStore = createTablePreferencesStore<HistoryColumnKey>({
  storageKey: 'dynamight-history-table-prefs',
  allColumns: HISTORY_ALL,
  fixedColumns: HISTORY_FIXED,
  defaultVisible: HISTORY_DEFAULT_VISIBLE,
  defaultWidths: HISTORY_DEFAULT_WIDTHS,
});

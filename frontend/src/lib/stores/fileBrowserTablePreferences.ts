import { createTablePreferencesStore } from './tablePreferences';

export type FileBrowserColumnKey = 'name' | 'size' | 'type' | 'modified' | 'actions';

export const FB_FIXED: FileBrowserColumnKey[] = ['name', 'actions'];
export const FB_ALL: FileBrowserColumnKey[] = ['name', 'type', 'size', 'modified', 'actions'];
export const FB_DEFAULT_VISIBLE: FileBrowserColumnKey[] = FB_ALL;
export const FB_DEFAULT_WIDTHS: Record<FileBrowserColumnKey, number> = {
  name: 300,
  type: 100,
  size: 120,
  modified: 180,
  actions: 80,
};

export const fileBrowserTablePreferencesStore = createTablePreferencesStore<FileBrowserColumnKey>({
  storageKey: 'dynamight-filebrowser-table-prefs',
  allColumns: FB_ALL,
  fixedColumns: FB_FIXED,
  defaultVisible: FB_DEFAULT_VISIBLE,
  defaultWidths: FB_DEFAULT_WIDTHS,
});

import { writable } from 'svelte/store';

type ViewMode = 'grid' | 'list';

const VIEW_MODE_KEY = 'dynamight-jobs-view';

function getInitialViewMode(): ViewMode {
  if (typeof window === 'undefined') return 'grid';

  const stored = localStorage.getItem(VIEW_MODE_KEY);
  if (stored === 'grid' || stored === 'list') {
    return stored;
  }

  return 'grid';
}

function createViewPreferencesStore() {
  const { subscribe, set } = writable<ViewMode>(getInitialViewMode());

  return {
    subscribe,
    setViewMode: (mode: ViewMode) => {
      set(mode);
      if (typeof window !== 'undefined') {
        localStorage.setItem(VIEW_MODE_KEY, mode);
      }
    }
  };
}

export const viewPreferencesStore = createViewPreferencesStore();

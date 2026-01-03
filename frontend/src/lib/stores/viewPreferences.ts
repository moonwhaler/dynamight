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
  const { subscribe, set, update } = writable<ViewMode>(getInitialViewMode());

  return {
    subscribe,
    setViewMode: (mode: ViewMode) => {
      set(mode);
      if (typeof window !== 'undefined') {
        localStorage.setItem(VIEW_MODE_KEY, mode);
      }
    },
    toggle: () => {
      update(current => {
        const next = current === 'grid' ? 'list' : 'grid';
        if (typeof window !== 'undefined') {
          localStorage.setItem(VIEW_MODE_KEY, next);
        }
        return next;
      });
    }
  };
}

export const viewPreferencesStore = createViewPreferencesStore();

import { writable } from 'svelte/store';

interface Preferences {
  showLogViewerAfterManualRun: boolean;
  autoShowLastPage: boolean;
  logRefreshInterval: number; // seconds
}

const STORAGE_KEY = 'dynamight-preferences';

const defaultPreferences: Preferences = {
  showLogViewerAfterManualRun: true,
  autoShowLastPage: true,
  logRefreshInterval: 2,
};

function loadPreferences(): Preferences {
  if (typeof window === 'undefined') return defaultPreferences;

  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      return { ...defaultPreferences, ...JSON.parse(stored) };
    }
  } catch {
    // Ignore parse errors
  }

  return defaultPreferences;
}

function savePreferences(prefs: Preferences): void {
  if (typeof window !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  }
}

function createPreferencesStore() {
  const { subscribe, update } = writable<Preferences>(loadPreferences());

  return {
    subscribe,
    setShowLogViewerAfterManualRun: (value: boolean) => {
      update(prefs => {
        const updated = { ...prefs, showLogViewerAfterManualRun: value };
        savePreferences(updated);
        return updated;
      });
    },
    setAutoShowLastPage: (value: boolean) => {
      update(prefs => {
        const updated = { ...prefs, autoShowLastPage: value };
        savePreferences(updated);
        return updated;
      });
    },
    setLogRefreshInterval: (value: number) => {
      update(prefs => {
        const updated = { ...prefs, logRefreshInterval: value };
        savePreferences(updated);
        return updated;
      });
    }
  };
}

export const preferencesStore = createPreferencesStore();

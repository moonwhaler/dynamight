import { writable } from 'svelte/store';

type Theme = 'light' | 'dark';

const THEME_KEY = 'dynamight-theme';

function getInitialTheme(): Theme {
  if (typeof window === 'undefined') return 'light';

  const stored = localStorage.getItem(THEME_KEY);
  if (stored === 'light' || stored === 'dark') {
    return stored;
  }

  // Check system preference
  if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    return 'dark';
  }

  return 'light';
}

function createThemeStore() {
  const { subscribe, set, update } = writable<Theme>(getInitialTheme());

  // Apply theme to document
  function applyTheme(theme: Theme) {
    if (typeof document === 'undefined') return;

    if (theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
    localStorage.setItem(THEME_KEY, theme);
  }

  // Initialize on load
  if (typeof window !== 'undefined') {
    const initial = getInitialTheme();
    applyTheme(initial);
  }

  return {
    subscribe,
    setTheme: (theme: Theme) => {
      set(theme);
      applyTheme(theme);
    },
    toggle: () => {
      update(current => {
        const next = current === 'light' ? 'dark' : 'light';
        applyTheme(next);
        return next;
      });
    }
  };
}

export const themeStore = createThemeStore();

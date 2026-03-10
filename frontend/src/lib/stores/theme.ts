import { writable, derived } from 'svelte/store';
import {
  type ThemeConfig,
  type BaseTheme,
  type AccentColor,
  DEFAULT_THEME,
  buildCssVars,
} from '../theme-presets';

const THEME_KEY = 'dynamight-theme';
const VARS_KEY = 'dynamight-theme-vars';

function loadConfig(): ThemeConfig {
  if (typeof window === 'undefined') return DEFAULT_THEME;

  try {
    const raw = localStorage.getItem(THEME_KEY);
    if (!raw) {
      // Check system dark preference for first-time users
      if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
        return { ...DEFAULT_THEME, mode: 'dark' };
      }
      return DEFAULT_THEME;
    }

    const parsed = JSON.parse(raw);

    // Backward compat: old store saved just "light" / "dark" as a plain string
    if (typeof parsed === 'string') {
      return { ...DEFAULT_THEME, mode: parsed as 'light' | 'dark' };
    }

    return {
      mode: parsed.mode ?? DEFAULT_THEME.mode,
      base: parsed.base ?? DEFAULT_THEME.base,
      accent: parsed.accent ?? DEFAULT_THEME.accent,
    };
  } catch {
    // If stored value is a bare string (not JSON), handle it
    const raw = localStorage.getItem(THEME_KEY);
    if (raw === 'light' || raw === 'dark') {
      return { ...DEFAULT_THEME, mode: raw };
    }
    return DEFAULT_THEME;
  }
}

function applyToDOM(config: ThemeConfig) {
  if (typeof document === 'undefined') return;

  // Dark / light class
  if (config.mode === 'dark') {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }

  // CSS variables
  const vars = buildCssVars(config);
  document.documentElement.setAttribute('style', vars);

  // Persist
  localStorage.setItem(THEME_KEY, JSON.stringify(config));
  localStorage.setItem(VARS_KEY, vars);
}

function createThemeStore() {
  const initial = loadConfig();
  const { subscribe, set, update } = writable<ThemeConfig>(initial);

  // Apply on load
  applyToDOM(initial);

  function applyAndSet(config: ThemeConfig) {
    applyToDOM(config);
    set(config);
  }

  return {
    subscribe,

    /** Toggle light ↔ dark */
    toggle: () => {
      update(current => {
        const next = { ...current, mode: (current.mode === 'light' ? 'dark' : 'light') as 'light' | 'dark' };
        applyToDOM(next);
        return next;
      });
    },

    /** Set light or dark mode */
    setMode: (mode: 'light' | 'dark') => {
      update(current => {
        const next = { ...current, mode };
        applyToDOM(next);
        return next;
      });
    },

    /** Set base tone */
    setBase: (base: BaseTheme) => {
      update(current => {
        const next = { ...current, base };
        applyToDOM(next);
        return next;
      });
    },

    /** Set accent color */
    setAccent: (accent: AccentColor) => {
      update(current => {
        const next = { ...current, accent };
        applyToDOM(next);
        return next;
      });
    },
  };
}

export const themeStore = createThemeStore();

// Derived stores for convenient access
export const themeMode = derived(themeStore, $t => $t.mode);
export const themeBase = derived(themeStore, $t => $t.base);
export const themeAccent = derived(themeStore, $t => $t.accent);

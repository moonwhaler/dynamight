import { writable } from 'svelte/store';
import { setLocale, getLocale, locales } from '$lib/paraglide/runtime.js';

// Type for available locales
export type Locale = (typeof locales)[number];

// Native language names for display
export const languageNames: Record<Locale, string> = {
  en: 'English',
  de: 'Deutsch',
};

function detectLanguage(): Locale {
  // Paraglide 2.0 handles detection via its strategy (cookie, localStorage, etc.)
  // We just return the current locale
  return getLocale() as Locale;
}

function createLanguageStore() {
  const initialLang = detectLanguage();

  const { subscribe, set } = writable<Locale>(initialLang);

  return {
    subscribe,
    setLanguage(lang: Locale) {
      setLocale(lang);
      set(lang);
    },
    get available(): readonly Locale[] {
      return locales;
    },
    get current(): Locale {
      return getLocale() as Locale;
    },
  };
}

export const languageStore = createLanguageStore();

// Re-export for convenience
export { locales };

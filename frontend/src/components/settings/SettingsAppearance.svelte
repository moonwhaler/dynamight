<script lang="ts">
  import { themeStore } from '../../lib/stores/theme';
  import { bases, accents, type BaseTheme, type AccentColor } from '../../lib/theme-presets';
  import { languageStore, languageNames, type Locale } from '../../lib/stores/language';
  import SettingsSection from './SettingsSection.svelte';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  const baseKeys = Object.keys(bases) as BaseTheme[];
  const accentKeys = Object.keys(accents) as AccentColor[];

  const baseLabels: Record<BaseTheme, () => string> = {
    paper: m.theme_base_paper,
    cloud: m.theme_base_cloud,
    slate: m.theme_base_slate,
  };
  const accentLabels: Record<AccentColor, () => string> = {
    ocean: m.theme_accent_ocean,
    emerald: m.theme_accent_emerald,
    rose: m.theme_accent_rose,
    amber: m.theme_accent_amber,
    violet: m.theme_accent_violet,
    teal: m.theme_accent_teal,
  };
</script>

<div class="space-y-6">
  <SettingsSection title={m.theme_title()} description={m.settings_appearance_desc()}>
    <div class="space-y-6">
      <fieldset>
        <legend class="eyebrow mb-2.5">{m.theme_mode()}</legend>
        <div class="inline-flex p-1 rounded-lg bg-gray-100 dark:bg-gray-900/60 gap-1" role="radiogroup">
          {#each [{ id: 'light', label: m.theme_light(), icon: 'sun' }, { id: 'dark', label: m.theme_dark(), icon: 'moon' }] as const as opt (opt.id)}
            <button
              type="button"
              role="radio"
              aria-checked={$themeStore.mode === opt.id}
              onclick={() => themeStore.setMode(opt.id)}
              class="inline-flex items-center gap-2 h-8 px-3.5 rounded-md text-sm font-medium transition-colors
                {$themeStore.mode === opt.id
                ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200'}"
            >
              <Icon name={opt.icon} class="w-4 h-4" />
              {opt.label}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend class="eyebrow mb-2.5">{m.theme_style()}</legend>
        <div class="grid grid-cols-3 gap-2 max-w-md" role="radiogroup">
          {#each baseKeys as key (key)}
            {@const active = $themeStore.base === key}
            <button
              type="button"
              role="radio"
              aria-checked={active}
              onclick={() => themeStore.setBase(key)}
              class="flex items-center gap-2.5 p-2.5 rounded-lg border text-left text-sm transition-colors
                {active
                ? 'border-primary-500 ring-1 ring-primary-500 text-gray-900 dark:text-white'
                : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'}"
            >
              <span class="w-6 h-6 rounded-full border border-gray-300 shrink-0" style="background-color: {bases[key].swatch}"></span>
              {baseLabels[key]()}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend class="eyebrow mb-2.5">{m.theme_accent()}</legend>
        <div class="flex flex-wrap gap-3" role="radiogroup">
          {#each accentKeys as key (key)}
            {@const active = $themeStore.accent === key}
            <button
              type="button"
              role="radio"
              aria-checked={active}
              aria-label={accentLabels[key]()}
              title={accentLabels[key]()}
              onclick={() => themeStore.setAccent(key)}
              class="w-9 h-9 rounded-full flex items-center justify-center transition-transform
                {active ? 'ring-2 ring-offset-2 ring-offset-white dark:ring-offset-gray-800 scale-105' : 'hover:scale-105'}"
              style="background-color: {accents[key].swatch}; --tw-ring-color: {accents[key].swatch}"
            >
              {#if active}<Icon name="check" class="w-4 h-4 text-white" strokeWidth={2.5} />{/if}
            </button>
          {/each}
        </div>
      </fieldset>
    </div>
  </SettingsSection>

  <SettingsSection title={m.settings_language_title()} description={m.settings_language_description()}>
    <label for="language" class="label">{m.settings_language_label()}</label>
    <select
      id="language"
      value={$languageStore}
      onchange={(e) => languageStore.setLanguage(e.currentTarget.value as Locale)}
      class="input max-w-xs"
    >
      {#each languageStore.available as lang (lang)}
        <option value={lang}>{languageNames[lang]}</option>
      {/each}
    </select>
  </SettingsSection>
</div>

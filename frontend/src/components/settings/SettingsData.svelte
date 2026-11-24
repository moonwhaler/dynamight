<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../../lib/api';
  import SettingsSection from './SettingsSection.svelte';
  import ConfigBackup from './ConfigBackup.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  type Key = 'max_runs_per_job' | 'delete_verification_window_minutes' | 'search_timeout_seconds';

  // Values as stored on the server, and the text in each input
  let stored = $state<Record<Key, number | null>>({
    max_runs_per_job: null,
    delete_verification_window_minutes: 5,
    search_timeout_seconds: 10,
  });
  let input = $state<Record<Key, string>>({
    max_runs_per_job: '',
    delete_verification_window_minutes: '5',
    search_timeout_seconds: '10',
  });
  let saving = $state<Key | 'sizes' | null>(null);
  let showDirectorySizes = $state(false);

  /** Allowed range per setting; max_runs_per_job may also be empty (unlimited) */
  const limits: Record<Key, { min: number; max: number; invalid: () => string }> = {
    max_runs_per_job: { min: 1, max: 1_000_000, invalid: m.settings_retention_invalid },
    delete_verification_window_minutes: { min: 1, max: 60, invalid: m.settings_delete_verification_invalid },
    search_timeout_seconds: { min: 3, max: 60, invalid: m.settings_search_timeout_invalid },
  };

  onMount(async () => {
    try {
      const s = await api.settings.get();
      stored = {
        max_runs_per_job: s.max_runs_per_job,
        delete_verification_window_minutes: s.delete_verification_window_minutes ?? 5,
        search_timeout_seconds: s.search_timeout_seconds ?? 10,
      };
      input = {
        max_runs_per_job: s.max_runs_per_job?.toString() ?? '',
        delete_verification_window_minutes: String(s.delete_verification_window_minutes ?? 5),
        search_timeout_seconds: String(s.search_timeout_seconds ?? 10),
      };
      showDirectorySizes = s.show_directory_sizes ?? false;
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.error_failed_load(), variant: 'error' });
    }
  });

  async function save(key: Key) {
    const text = input[key].trim();
    const value = text === '' && key === 'max_runs_per_job' ? null : parseInt(text, 10);
    const { min, max, invalid } = limits[key];
    if (value !== null && (Number.isNaN(value) || value < min || value > max)) {
      showToast({ message: invalid(), variant: 'error' });
      input[key] = stored[key]?.toString() ?? '';
      return;
    }
    if (value === stored[key]) return;
    saving = key;
    try {
      await api.settings.update({ [key]: value });
      stored[key] = value;
      showToast({ message: m.settings_saved(), variant: 'success' });
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_save_error(), variant: 'error' });
      input[key] = stored[key]?.toString() ?? '';
    } finally {
      saving = null;
    }
  }

  async function saveSizes(checked: boolean) {
    saving = 'sizes';
    try {
      await api.settings.update({ show_directory_sizes: checked });
      showToast({ message: m.settings_saved(), variant: 'success' });
    } catch (err) {
      showDirectorySizes = !checked;
      showToast({ message: err instanceof Error ? err.message : m.settings_save_error(), variant: 'error' });
    } finally {
      saving = null;
    }
  }
</script>

{#snippet numberField(key: Key, id: string, label: string, help: string, unit: string, placeholder = '')}
  <div>
    <label for={id} class="label">{label}</label>
    <div class="flex items-center gap-2">
      <input
        {id}
        type="text"
        inputmode="numeric"
        bind:value={input[key]}
        onchange={() => save(key)}
        disabled={saving === key}
        {placeholder}
        class="input w-28 tabular-nums"
      />
      <span class="text-sm text-gray-500 dark:text-gray-400">{unit}</span>
      {#if saving === key}<span class="text-primary-600"><Spinner /></span>{/if}
    </div>
    <p class="hint">{help}</p>
  </div>
{/snippet}

<div class="space-y-6">
  <SettingsSection title={m.settings_retention_title()} description={m.settings_retention_description()}>
    {@render numberField(
      'max_runs_per_job',
      'max-runs',
      m.settings_retention_max_runs(),
      m.settings_retention_help(),
      m.settings_retention_unit(),
      m.settings_retention_placeholder(),
    )}
  </SettingsSection>

  <SettingsSection title={m.settings_filebrowser_title()}>
    <div class="space-y-6">
      <Toggle
        label={m.settings_show_directory_sizes()}
        description={m.settings_show_directory_sizes_desc()}
        bind:checked={showDirectorySizes}
        disabled={saving === 'sizes'}
        onchange={saveSizes}
      />
      {@render numberField(
        'delete_verification_window_minutes',
        'delete-window',
        m.settings_delete_verification_label(),
        m.settings_delete_verification_desc(),
        m.settings_delete_verification_minutes(),
      )}
      {@render numberField(
        'search_timeout_seconds',
        'search-timeout',
        m.settings_search_timeout_label(),
        m.settings_search_timeout_desc(),
        m.settings_search_timeout_seconds(),
      )}
    </div>
  </SettingsSection>

  <ConfigBackup />
</div>

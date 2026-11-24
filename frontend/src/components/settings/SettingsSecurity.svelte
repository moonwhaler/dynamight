<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../../lib/api';
  import type { TotpStatusResponse } from '../../lib/types';
  import TotpSetup from '../TotpSetup.svelte';
  import SettingsSection from './SettingsSection.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Icon from '../ui/Icon.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let status = $state<TotpStatusResponse | null>(null);
  let loading = $state(true);
  let password = $state('');
  let code = $state('');
  let disabling = $state(false);

  async function load() {
    loading = true;
    try {
      status = await api.auth.totpStatus();
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_2fa_load_error(), variant: 'error' });
    }
    loading = false;
  }

  async function disable(e: Event) {
    e.preventDefault();
    disabling = true;
    try {
      await api.auth.totpDisable(password, code);
      status = { enabled: false, recovery_codes_remaining: 0 };
      password = '';
      code = '';
      showToast({ message: m.settings_2fa_disabled(), variant: 'success' });
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_2fa_disable_error(), variant: 'error' });
    }
    disabling = false;
  }

  onMount(load);
</script>

<SettingsSection title={m.settings_2fa_title()} description={m.settings_2fa_description()}>
  {#if loading}
    <div class="flex justify-center py-8 text-primary-600"><Spinner size="w-6 h-6" /></div>
  {:else if status?.enabled}
    <div class="space-y-5">
      <div class="flex items-center gap-3 p-4 rounded-lg bg-green-50 dark:bg-green-900/20">
        <span class="text-green-600 dark:text-green-400"><Icon name="shield" class="w-6 h-6" /></span>
        <div>
          <p class="font-medium text-green-800 dark:text-green-300">{m.settings_2fa_enabled()}</p>
          <p class="text-sm text-green-700 dark:text-green-400">{m.settings_2fa_recovery_remaining({ count: status.recovery_codes_remaining })}</p>
        </div>
      </div>

      {#if status.recovery_codes_remaining < 3}
        <p class="p-3 rounded-lg bg-amber-50 dark:bg-amber-900/20 text-sm text-amber-800 dark:text-amber-300">{m.settings_2fa_low_codes()}</p>
      {/if}

      <form onsubmit={disable} class="space-y-4 pt-5 border-t border-gray-200 dark:border-gray-700">
        <div>
          <p class="text-sm font-medium text-gray-900 dark:text-white">{m.settings_2fa_disable_title()}</p>
          <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.settings_2fa_disable_description()}</p>
        </div>
        <div class="grid gap-4 sm:grid-cols-2 max-w-xl">
          <div>
            <label for="disable-password" class="label">{m.auth_password()}</label>
            <input id="disable-password" type="password" bind:value={password} class="input" autocomplete="current-password" />
          </div>
          <div>
            <label for="disable-code" class="label">{m.settings_2fa_authenticator_code()}</label>
            <input
              id="disable-code"
              type="text"
              inputmode="numeric"
              pattern="[0-9]*"
              maxlength="6"
              bind:value={code}
              class="input font-mono tracking-widest"
              placeholder="000000"
              autocomplete="one-time-code"
            />
          </div>
        </div>
        <button type="submit" disabled={disabling || !password || code.length !== 6} class="btn btn-danger">
          {#if disabling}<Spinner />{m.settings_2fa_disabling()}{:else}{m.settings_2fa_disable()}{/if}
        </button>
      </form>
    </div>
  {:else}
    <TotpSetup onEnabled={load} />
  {/if}
</SettingsSection>

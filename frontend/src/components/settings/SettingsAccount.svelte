<script lang="ts">
  import { api } from '../../lib/api';
  import { authStore } from '../../lib/stores/auth';
  import PasswordStrength from '../PasswordStrength.svelte';
  import SettingsSection from './SettingsSection.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Icon from '../ui/Icon.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let saving = $state(false);

  async function submit(e: Event) {
    e.preventDefault();
    if (newPassword.length < 8) {
      showToast({ message: m.error_password_too_short(), variant: 'error' });
      return;
    }
    if (newPassword !== confirmPassword) {
      showToast({ message: m.error_passwords_mismatch(), variant: 'error' });
      return;
    }
    if (currentPassword === newPassword) {
      showToast({ message: m.error_password_same(), variant: 'error' });
      return;
    }
    saving = true;
    try {
      await api.auth.changePassword(currentPassword, newPassword);
      showToast({ message: m.settings_password_changed(), variant: 'success' });
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_password_error(), variant: 'error' });
    } finally {
      saving = false;
    }
  }
</script>

<div class="space-y-6">
  <SettingsSection title={m.settings_signed_in()}>
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3 min-w-0">
        <span class="w-10 h-10 rounded-full bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300 flex items-center justify-center font-semibold uppercase">
          {$authStore.user?.username?.slice(0, 1) ?? '?'}
        </span>
        <span class="font-medium text-gray-900 dark:text-white truncate">{$authStore.user?.username}</span>
      </div>
      <button type="button" class="btn btn-secondary" onclick={() => authStore.logout()}>
        <Icon name="logout" class="w-4 h-4" />
        {m.auth_logout()}
      </button>
    </div>
  </SettingsSection>

  <SettingsSection title={m.settings_password_title()} description={m.settings_password_description()}>
    <form onsubmit={submit} class="space-y-4 max-w-xl">
      <div>
        <label for="current-password" class="label">{m.settings_current_password()}</label>
        <input id="current-password" type="password" required bind:value={currentPassword} class="input" autocomplete="current-password" />
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <div>
          <label for="new-password" class="label">{m.settings_new_password()}</label>
          <input
            id="new-password"
            type="password"
            required
            bind:value={newPassword}
            class="input"
            placeholder={m.settings_new_password_placeholder()}
            autocomplete="new-password"
          />
          <PasswordStrength password={newPassword} />
        </div>
        <div>
          <label for="confirm-password" class="label">{m.settings_confirm_password()}</label>
          <input id="confirm-password" type="password" required bind:value={confirmPassword} class="input" autocomplete="new-password" />
        </div>
      </div>
      <button type="submit" disabled={saving} class="btn btn-primary">
        {#if saving}<Spinner />{m.settings_updating_password()}{:else}{m.settings_update_password()}{/if}
      </button>
    </form>
  </SettingsSection>
</div>

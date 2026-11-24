<script lang="ts">
  import Logo from '../components/shell/Logo.svelte';
  import { authStore } from '../lib/stores/auth';
  import PasswordStrength from '../components/PasswordStrength.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let mode = $state<'create' | 'restore'>('create');
  let username = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let loading = $state(false);
  let validationError = $state('');

  // Restore mode state
  let backupFile = $state<File | null>(null);
  let backupPassword = $state('');
  let restoreSuccess = $state(false);

  function validate(): boolean {
    validationError = '';

    if (username.trim().length < 3) {
      validationError = m.error_field_required({ field: m.auth_username() });
      return false;
    }

    if (password.length < 8) {
      validationError = m.error_password_too_short();
      return false;
    }

    if (password !== confirmPassword) {
      validationError = m.error_passwords_mismatch();
      return false;
    }

    return true;
  }

  function validateRestore(): boolean {
    validationError = '';

    if (!backupFile) {
      validationError = m.error_field_required({ field: m.setup_restore_file() });
      return false;
    }

    if (backupPassword.length < 8) {
      validationError = m.error_password_too_short();
      return false;
    }

    return true;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!validate()) return;

    loading = true;
    const success = await authStore.setup(username.trim(), password);
    loading = false;

    if (success) {
      // Navigate to dashboard after successful setup
      window.location.hash = '#/';
    }
  }

  async function handleRestore(e: Event) {
    e.preventDefault();
    if (!validateRestore()) return;

    loading = true;
    const success = await authStore.setupFromBackup(backupFile!, backupPassword);
    loading = false;

    if (success) {
      restoreSuccess = true;
    }
  }

  function handleFileChange(e: Event) {
    const input = e.target as HTMLInputElement;
    backupFile = input.files?.[0] ?? null;
  }

  function switchMode(newMode: 'create' | 'restore') {
    mode = newMode;
    validationError = '';
    restoreSuccess = false;
    authStore.clearError();
  }
</script>

<div class="min-h-[100dvh] flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 pt-safe pb-safe">
  <div class="max-w-md w-full space-y-8">
    <div>
      <div class="flex justify-center mb-10"><Logo large /></div>
      <h1 class="font-display text-[1.9rem] leading-tight text-gray-900 dark:text-white">{m.setup_title()}</h1>
      <p class="mt-1.5 text-sm text-gray-500 dark:text-gray-400">{m.setup_description()}</p>
    </div>

    <!-- Mode toggle -->
    <div class="grid grid-cols-2 gap-3">
      <button
        type="button"
        onclick={() => switchMode('create')}
        class="flex items-center justify-center gap-2 px-4 py-3 rounded-lg border-2 text-sm font-medium transition-colors {mode === 'create' ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'}"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z" />
        </svg>
        {m.setup_mode_create()}
      </button>
      <button
        type="button"
        onclick={() => switchMode('restore')}
        class="flex items-center justify-center gap-2 px-4 py-3 rounded-lg border-2 text-sm font-medium transition-colors {mode === 'restore' ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'}"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
        </svg>
        {m.setup_mode_restore()}
      </button>
    </div>

    {#if mode === 'create'}
      <!-- Create account form (existing) -->
      <form class="mt-8 space-y-6" onsubmit={handleSubmit}>
        {#if validationError || $authStore.error}
          <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
            {validationError || $authStore.error}
          </div>
        {/if}

        <div class="space-y-4">
          <div>
            <label for="username" class="label">{m.setup_username()}</label>
            <input
              id="username"
              name="username"
              type="text"
              required
              bind:value={username}
              class="input"
              placeholder={m.setup_username_placeholder()}
              autocomplete="username"
            />
          </div>

          <div>
            <label for="password" class="label">{m.setup_password()}</label>
            <input
              id="password"
              name="password"
              type="password"
              required
              bind:value={password}
              class="input"
              placeholder={m.setup_password_placeholder()}
              autocomplete="new-password"
            />
            <PasswordStrength {password} />
          </div>

          <div>
            <label for="confirmPassword" class="label">{m.setup_confirm_password()}</label>
            <input
              id="confirmPassword"
              name="confirmPassword"
              type="password"
              required
              bind:value={confirmPassword}
              class="input"
              placeholder={m.setup_confirm_password_placeholder()}
              autocomplete="new-password"
            />
          </div>
        </div>

        <button type="submit" disabled={loading} class="w-full btn btn-lg btn-primary">
          {#if loading}
            <span class="flex items-center justify-center gap-2">
              <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
                <circle
                  class="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  stroke-width="4"
                  fill="none"
                />
                <path
                  class="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                />
              </svg>
              {m.setup_creating()}
            </span>
          {:else}
            {m.setup_create_account()}
          {/if}
        </button>
      </form>

      <div class="text-center">
        <p class="text-xs text-gray-500 dark:text-gray-400">
          {m.setup_admin_note()}
        </p>
      </div>
    {:else}
      <!-- Restore from backup form -->
      {#if restoreSuccess}
        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-700 dark:text-green-400 px-4 py-3 rounded-lg text-sm">
          {m.setup_restore_success()}
        </div>
      {:else}
        <form class="mt-8 space-y-6" onsubmit={handleRestore}>
          {#if validationError || $authStore.error}
            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
              {validationError || $authStore.error}
            </div>
          {/if}

          <p class="text-sm text-gray-600 dark:text-gray-400">
            {m.setup_restore_description()}
          </p>

          <div class="space-y-4">
            <div>
              <label for="backupFile" class="label">{m.setup_restore_file()}</label>
              <input
                id="backupFile"
                name="backupFile"
                type="file"
                accept=".dmbackup"
                onchange={handleFileChange}
                class="input"
              />
            </div>

            <div>
              <label for="backupPassword" class="label">{m.setup_restore_password()}</label>
              <input
                id="backupPassword"
                name="backupPassword"
                type="password"
                bind:value={backupPassword}
                class="input"
                placeholder={m.setup_restore_password_placeholder()}
              />
            </div>
          </div>

          <button type="submit" disabled={loading || !backupFile} class="w-full btn btn-lg btn-primary">
            {#if loading}
              <span class="flex items-center justify-center gap-2">
                <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
                  <circle
                    class="opacity-25"
                    cx="12"
                    cy="12"
                    r="10"
                    stroke="currentColor"
                    stroke-width="4"
                    fill="none"
                  />
                  <path
                    class="opacity-75"
                    fill="currentColor"
                    d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                  />
                </svg>
                {m.setup_restoring()}
              </span>
            {:else}
              {m.setup_restore_button()}
            {/if}
          </button>
        </form>
      {/if}
    {/if}
  </div>
</div>

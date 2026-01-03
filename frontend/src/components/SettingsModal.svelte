<script lang="ts">
  import { api } from '../lib/api';
  import PasswordStrength from './PasswordStrength.svelte';

  let { open = $bindable(false) } = $props();

  type Tab = 'account' | 'logs';
  let activeTab = $state<Tab>('account');

  // Password change state
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let passwordLoading = $state(false);
  let passwordError = $state('');
  let passwordSuccess = $state(false);

  // Logs settings state
  let maxRunsPerJob = $state<number | null>(null);
  let maxRunsInput = $state('');
  let logsLoading = $state(false);
  let logsError = $state('');
  let logsSaved = $state(false);
  let initialMaxRuns = $state<number | null>(null);

  function resetPasswordForm() {
    currentPassword = '';
    newPassword = '';
    confirmPassword = '';
    passwordError = '';
    passwordSuccess = false;
  }

  function close() {
    open = false;
    activeTab = 'account';
    resetPasswordForm();
    logsError = '';
    logsSaved = false;
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      close();
    }
  }

  async function loadSettings() {
    try {
      const settings = await api.settings.get();
      maxRunsPerJob = settings.max_runs_per_job;
      initialMaxRuns = settings.max_runs_per_job;
      maxRunsInput = settings.max_runs_per_job?.toString() ?? '';
    } catch (err) {
      console.error('Failed to load settings:', err);
    }
  }

  $effect(() => {
    if (open) {
      loadSettings();
    }
  });

  async function handlePasswordSubmit(e: Event) {
    e.preventDefault();
    passwordError = '';

    if (newPassword.length < 8) {
      passwordError = 'New password must be at least 8 characters';
      return;
    }

    if (newPassword !== confirmPassword) {
      passwordError = 'Passwords do not match';
      return;
    }

    if (currentPassword === newPassword) {
      passwordError = 'New password must be different from current password';
      return;
    }

    passwordLoading = true;
    try {
      await api.auth.changePassword(currentPassword, newPassword);
      passwordSuccess = true;
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      passwordError = err instanceof Error ? err.message : 'Failed to change password';
    } finally {
      passwordLoading = false;
    }
  }

  async function handleLogsSave() {
    logsError = '';
    logsSaved = false;

    const value = maxRunsInput.trim() === '' ? null : parseInt(maxRunsInput, 10);

    if (value !== null && (isNaN(value) || value < 1)) {
      logsError = 'Please enter a valid number (minimum 1) or leave empty for unlimited';
      return;
    }

    logsLoading = true;
    try {
      await api.settings.update({ max_runs_per_job: value });
      maxRunsPerJob = value;
      initialMaxRuns = value;
      logsSaved = true;
      setTimeout(() => logsSaved = false, 3000);
    } catch (err) {
      logsError = err instanceof Error ? err.message : 'Failed to save settings';
    } finally {
      logsLoading = false;
    }
  }

  function switchTab(tab: Tab) {
    activeTab = tab;
    if (tab === 'account') {
      resetPasswordForm();
    }
    logsError = '';
    logsSaved = false;
  }

  let hasLogsChanges = $derived(() => {
    const currentValue = maxRunsInput.trim() === '' ? null : parseInt(maxRunsInput, 10);
    if (isNaN(currentValue as number)) return false;
    return currentValue !== initialMaxRuns;
  });
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-title"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden">
      <!-- Header -->
      <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <h2 id="settings-title" class="text-lg font-semibold text-gray-900 dark:text-white">Settings</h2>
        <button
          onclick={close}
          class="p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
          aria-label="Close"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Tabs -->
      <div class="border-b border-gray-200 dark:border-gray-700">
        <nav class="flex px-6" aria-label="Settings tabs">
          <button
            onclick={() => switchTab('account')}
            class="relative px-4 py-3 text-sm font-medium transition-colors {activeTab === 'account'
              ? 'text-primary-600 dark:text-primary-400'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
            aria-selected={activeTab === 'account'}
            role="tab"
          >
            <span class="flex items-center gap-2">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
              </svg>
              Account
            </span>
            {#if activeTab === 'account'}
              <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary-600 dark:bg-primary-400 rounded-full"></span>
            {/if}
          </button>
          <button
            onclick={() => switchTab('logs')}
            class="relative px-4 py-3 text-sm font-medium transition-colors {activeTab === 'logs'
              ? 'text-primary-600 dark:text-primary-400'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
            aria-selected={activeTab === 'logs'}
            role="tab"
          >
            <span class="flex items-center gap-2">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              History
            </span>
            {#if activeTab === 'logs'}
              <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary-600 dark:bg-primary-400 rounded-full"></span>
            {/if}
          </button>
        </nav>
      </div>

      <!-- Content -->
      <div class="p-6">
        {#if activeTab === 'account'}
          <!-- Account Tab -->
          {#if passwordSuccess}
            <div class="text-center py-6">
              <div class="w-14 h-14 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center mx-auto mb-4">
                <svg class="w-7 h-7 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">Password Changed</h3>
              <p class="text-sm text-gray-600 dark:text-gray-400 mb-6">Your password has been updated successfully.</p>
              <button onclick={() => passwordSuccess = false} class="btn btn-primary">Change Another</button>
            </div>
          {:else}
            <div class="mb-5">
              <h3 class="text-base font-medium text-gray-900 dark:text-white mb-1">Change Password</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400">Update your account password</p>
            </div>

            <form onsubmit={handlePasswordSubmit} class="space-y-4">
              {#if passwordError}
                <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
                  {passwordError}
                </div>
              {/if}

              <div>
                <label for="current-password" class="label">Current Password</label>
                <input
                  id="current-password"
                  type="password"
                  required
                  bind:value={currentPassword}
                  class="input"
                  placeholder="Enter current password"
                  autocomplete="current-password"
                />
              </div>

              <div>
                <label for="new-password" class="label">New Password</label>
                <input
                  id="new-password"
                  type="password"
                  required
                  bind:value={newPassword}
                  class="input"
                  placeholder="Enter new password"
                  autocomplete="new-password"
                />
                <PasswordStrength password={newPassword} />
                {#if !newPassword}
                  <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Must be at least 8 characters</p>
                {/if}
              </div>

              <div>
                <label for="confirm-password" class="label">Confirm New Password</label>
                <input
                  id="confirm-password"
                  type="password"
                  required
                  bind:value={confirmPassword}
                  class="input"
                  placeholder="Confirm new password"
                  autocomplete="new-password"
                />
              </div>

              <div class="pt-2">
                <button type="submit" disabled={passwordLoading} class="btn btn-primary w-full">
                  {#if passwordLoading}
                    <span class="flex items-center justify-center gap-2">
                      <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
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
                      Updating...
                    </span>
                  {:else}
                    Update Password
                  {/if}
                </button>
              </div>
            </form>
          {/if}

        {:else if activeTab === 'logs'}
          <!-- History Tab -->
          <div class="mb-5">
            <h3 class="text-base font-medium text-gray-900 dark:text-white mb-1">History Retention</h3>
            <p class="text-sm text-gray-500 dark:text-gray-400">Control how many job runs are kept in history</p>
          </div>

          <div class="space-y-4">
            {#if logsError}
              <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
                {logsError}
              </div>
            {/if}

            {#if logsSaved}
              <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-700 dark:text-green-400 px-4 py-3 rounded-lg text-sm flex items-center gap-2">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
                Settings saved successfully
              </div>
            {/if}

            <div>
              <label for="max-runs" class="label">Maximum Runs per Job</label>
              <div class="relative">
                <input
                  id="max-runs"
                  type="text"
                  inputmode="numeric"
                  bind:value={maxRunsInput}
                  class="input pr-20"
                  placeholder="5"
                />
                <span class="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-gray-400 dark:text-gray-500">runs</span>
              </div>
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">
                Older job runs and their logs will be automatically deleted when this limit is exceeded. Default is 5 runs per job.
              </p>
            </div>

            <div class="bg-gray-50 dark:bg-gray-900/50 rounded-lg p-4">
              <div class="flex items-start gap-3">
                <div class="flex-shrink-0 w-8 h-8 bg-primary-100 dark:bg-primary-900/30 rounded-lg flex items-center justify-center">
                  <svg class="w-4 h-4 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                </div>
                <div class="text-sm">
                  <p class="font-medium text-gray-900 dark:text-white">Current Setting</p>
                  <p class="text-gray-500 dark:text-gray-400 mt-0.5">
                    {#if maxRunsPerJob === null}
                      Unlimited history retention
                    {:else}
                      Keeping last {maxRunsPerJob} run{maxRunsPerJob === 1 ? '' : 's'} per job
                    {/if}
                  </p>
                </div>
              </div>
            </div>

            <div class="pt-2">
              <button
                onclick={handleLogsSave}
                disabled={logsLoading || !hasLogsChanges()}
                class="btn btn-primary w-full disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {#if logsLoading}
                  <span class="flex items-center justify-center gap-2">
                    <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
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
                    Saving...
                  </span>
                {:else}
                  Save Changes
                {/if}
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

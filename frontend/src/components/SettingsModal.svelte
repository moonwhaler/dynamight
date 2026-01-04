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

  const tabs = [
    {
      id: 'account' as Tab,
      label: 'Account',
      description: 'Manage your password',
      icon: 'M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z'
    },
    {
      id: 'logs' as Tab,
      label: 'History',
      description: 'Configure retention',
      icon: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z'
    }
  ];
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-title"
    tabindex="-1"
  >
    <!-- Large dialog using viewport units like LogViewer -->
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-5xl h-[90vh] md:h-[85vh] flex flex-col overflow-hidden">
      <!-- Header -->
      <div class="px-6 py-5 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div>
          <h2 id="settings-title" class="text-xl font-semibold text-gray-900 dark:text-white">Settings</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">Manage your account and preferences</p>
        </div>
        <button
          onclick={close}
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-xl transition-colors"
          aria-label="Close"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Main Content Area -->
      <div class="flex flex-col md:flex-row flex-1 min-h-0 overflow-hidden">
        <!-- Sidebar Navigation (Desktop) / Top Tabs (Mobile) -->
        <nav class="md:w-64 md:border-r border-b md:border-b-0 border-gray-200 dark:border-gray-700 flex-shrink-0 bg-gray-50/50 dark:bg-gray-900/30">
          <!-- Mobile: Horizontal tabs -->
          <div class="flex md:hidden p-2 gap-1">
            {#each tabs as tab}
              <button
                onclick={() => switchTab(tab.id)}
                class="flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl text-sm font-medium transition-all {activeTab === tab.id
                  ? 'bg-white dark:bg-gray-700 text-primary-600 dark:text-primary-400 shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-white/50 dark:hover:bg-gray-700/50'}"
                aria-selected={activeTab === tab.id}
                role="tab"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={tab.icon} />
                </svg>
                {tab.label}
              </button>
            {/each}
          </div>

          <!-- Desktop: Vertical navigation -->
          <div class="hidden md:flex flex-col p-3 gap-1">
            {#each tabs as tab}
              <button
                onclick={() => switchTab(tab.id)}
                class="flex items-center gap-3 w-full h-16 px-4 rounded-xl text-left transition-all {activeTab === tab.id
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-white/50 dark:hover:bg-gray-700/50'}"
                aria-selected={activeTab === tab.id}
                role="tab"
              >
                <div class="w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 {activeTab === tab.id
                  ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-600 dark:text-primary-400'
                  : 'bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400'}">
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={tab.icon} />
                  </svg>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="font-medium text-sm">{tab.label}</div>
                  <div class="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">{tab.description}</div>
                </div>
              </button>
            {/each}
          </div>
        </nav>

        <!-- Content Panel - scrollable -->
        <div class="flex-1 overflow-y-auto bg-gray-50/30 dark:bg-gray-900/20">
          <div class="p-6 md:p-10 lg:p-12">
            {#if activeTab === 'account'}
              <!-- Account Tab -->
              {#if passwordSuccess}
                <div class="flex flex-col items-center justify-center py-16 md:py-24">
                  <div class="w-20 h-20 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center mb-6">
                    <svg class="w-10 h-10 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                    </svg>
                  </div>
                  <h3 class="text-2xl font-semibold text-gray-900 dark:text-white mb-3">Password Changed</h3>
                  <p class="text-gray-600 dark:text-gray-400 mb-10 text-center max-w-md text-lg">Your password has been updated successfully. Use your new password next time you log in.</p>
                  <button onclick={() => passwordSuccess = false} class="btn btn-primary px-8 py-2.5">Change Another Password</button>
                </div>
              {:else}
                <!-- Page header -->
                <div class="mb-10">
                  <h3 class="text-2xl font-semibold text-gray-900 dark:text-white mb-2">Change Password</h3>
                  <p class="text-gray-500 dark:text-gray-400 text-lg">Update your account password. Choose a strong password that you don't use elsewhere.</p>
                </div>

                <!-- Content card -->
                <div class="bg-white dark:bg-gray-800 rounded-2xl border border-gray-200 dark:border-gray-700 shadow-sm">
                  <form onsubmit={handlePasswordSubmit}>
                    {#if passwordError}
                      <div class="mx-6 md:mx-8 mt-6 md:mt-8 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-5 py-4 rounded-xl flex items-start gap-4">
                        <svg class="w-6 h-6 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span class="text-sm">{passwordError}</span>
                      </div>
                    {/if}

                    <div class="p-6 md:p-8 space-y-6">
                      <div class="grid gap-6 md:grid-cols-2">
                        <div class="md:col-span-2">
                          <label for="current-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Current Password</label>
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
                          <label for="new-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">New Password</label>
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
                            <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">Must be at least 8 characters</p>
                          {/if}
                        </div>

                        <div>
                          <label for="confirm-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Confirm New Password</label>
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
                      </div>
                    </div>

                    <div class="px-6 md:px-8 py-5 bg-gray-50 dark:bg-gray-900/50 border-t border-gray-200 dark:border-gray-700 rounded-b-2xl flex justify-end">
                      <button type="submit" disabled={passwordLoading} class="btn btn-primary px-8 py-2.5">
                        {#if passwordLoading}
                          <span class="flex items-center justify-center gap-2">
                            <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                            </svg>
                            Updating...
                          </span>
                        {:else}
                          Update Password
                        {/if}
                      </button>
                    </div>
                  </form>
                </div>
              {/if}

            {:else if activeTab === 'logs'}
              <!-- History Tab -->
              <!-- Page header -->
              <div class="mb-10">
                <h3 class="text-2xl font-semibold text-gray-900 dark:text-white mb-2">History Retention</h3>
                <p class="text-gray-500 dark:text-gray-400 text-lg">Control how many job runs are kept in history. Older runs will be automatically deleted.</p>
              </div>

              <!-- Alerts -->
              {#if logsError}
                <div class="mb-6 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-5 py-4 rounded-xl flex items-start gap-4">
                  <svg class="w-6 h-6 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>{logsError}</span>
                </div>
              {/if}

              {#if logsSaved}
                <div class="mb-6 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-700 dark:text-green-400 px-5 py-4 rounded-xl flex items-center gap-4">
                  <svg class="w-6 h-6 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                  <span>Settings saved successfully</span>
                </div>
              {/if}

              <!-- Content cards -->
              <div class="grid gap-6 lg:grid-cols-2">
                <!-- Settings card -->
                <div class="bg-white dark:bg-gray-800 rounded-2xl border border-gray-200 dark:border-gray-700 shadow-sm flex flex-col">
                  <div class="p-6 md:p-8 flex-1">
                    <div class="flex items-center gap-4 mb-6">
                      <div class="w-12 h-12 bg-primary-100 dark:bg-primary-900/40 rounded-xl flex items-center justify-center">
                        <svg class="w-6 h-6 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
                        </svg>
                      </div>
                      <div>
                        <h4 class="font-semibold text-gray-900 dark:text-white">Retention Limit</h4>
                        <p class="text-sm text-gray-500 dark:text-gray-400">Set max runs per job</p>
                      </div>
                    </div>

                    <div>
                      <label for="max-runs" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Maximum Runs per Job</label>
                      <div class="relative">
                        <input
                          id="max-runs"
                          type="text"
                          inputmode="numeric"
                          bind:value={maxRunsInput}
                          class="input pr-16 text-lg"
                          placeholder="5"
                        />
                        <span class="absolute right-4 top-1/2 -translate-y-1/2 text-sm text-gray-400 dark:text-gray-500">runs</span>
                      </div>
                      <p class="text-sm text-gray-500 dark:text-gray-400 mt-3">
                        Leave empty for unlimited retention.
                      </p>
                    </div>
                  </div>

                  <div class="px-6 md:px-8 py-5 bg-gray-50 dark:bg-gray-900/50 border-t border-gray-200 dark:border-gray-700 rounded-b-2xl flex justify-end mt-auto">
                    <button
                      onclick={handleLogsSave}
                      disabled={logsLoading || !hasLogsChanges()}
                      class="btn btn-primary px-8 py-2.5 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {#if logsLoading}
                        <span class="flex items-center justify-center gap-2">
                          <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                          </svg>
                          Saving...
                        </span>
                      {:else}
                        Save Changes
                      {/if}
                    </button>
                  </div>
                </div>

                <!-- Info card -->
                <div class="bg-white dark:bg-gray-800 rounded-2xl border border-gray-200 dark:border-gray-700 shadow-sm p-6 md:p-8">
                  <div class="flex items-center gap-4 mb-6">
                    <div class="w-12 h-12 bg-blue-100 dark:bg-blue-900/40 rounded-xl flex items-center justify-center">
                      <svg class="w-6 h-6 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                    </div>
                    <div>
                      <h4 class="font-semibold text-gray-900 dark:text-white">Current Status</h4>
                      <p class="text-sm text-gray-500 dark:text-gray-400">Active retention setting</p>
                    </div>
                  </div>

                  <div class="space-y-4">
                    <div class="flex items-center justify-between py-3 border-b border-gray-100 dark:border-gray-700">
                      <span class="text-gray-600 dark:text-gray-400">Retention Mode</span>
                      <span class="font-medium text-gray-900 dark:text-white">
                        {#if maxRunsPerJob === null}
                          Unlimited
                        {:else}
                          Limited
                        {/if}
                      </span>
                    </div>
                    <div class="flex items-center justify-between py-3 border-b border-gray-100 dark:border-gray-700">
                      <span class="text-gray-600 dark:text-gray-400">Runs Kept</span>
                      <span class="font-medium text-gray-900 dark:text-white">
                        {#if maxRunsPerJob === null}
                          All runs
                        {:else}
                          Last {maxRunsPerJob} per job
                        {/if}
                      </span>
                    </div>
                    <div class="flex items-center justify-between py-3">
                      <span class="text-gray-600 dark:text-gray-400">Auto Cleanup</span>
                      <span class="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium {maxRunsPerJob === null ? 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400' : 'bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-400'}">
                        {maxRunsPerJob === null ? 'Disabled' : 'Enabled'}
                      </span>
                    </div>
                  </div>

                  <div class="mt-6 p-4 bg-amber-50 dark:bg-amber-900/20 rounded-xl border border-amber-100 dark:border-amber-800/50">
                    <p class="text-sm text-amber-800 dark:text-amber-300">
                      <strong>Note:</strong> When a limit is set, older job runs and their logs are automatically deleted when new runs exceed the threshold.
                    </p>
                  </div>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

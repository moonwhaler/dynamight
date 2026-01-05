<script lang="ts">
  import { api } from '../lib/api';
  import PasswordStrength from './PasswordStrength.svelte';
  import TotpSetup from './TotpSetup.svelte';
  import CredentialsManager from './settings/CredentialsManager.svelte';
  import type { TotpStatusResponse } from '../lib/types';
  import { preferencesStore } from '../lib/stores/preferences';
  import { showToast } from './ui/Toast.svelte';
  import { languageStore, languageNames, type Locale } from '../lib/stores/language';
  import * as m from '$lib/paraglide/messages.js';

  let { open = $bindable(false) } = $props();

  type Tab = 'general' | 'account' | 'security' | 'credentials' | 'logs';
  let activeTab = $state<Tab>('general');

  // 2FA state
  let totpStatus = $state<TotpStatusResponse | null>(null);
  let totpLoading = $state(false);
  let disablePassword = $state('');
  let disableCode = $state('');
  let disableLoading = $state(false);

  // Password change state
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let passwordLoading = $state(false);

  // Logs settings state
  let maxRunsPerJob = $state<number | null>(null);
  let maxRunsInput = $state('');
  let logsLoading = $state(false);
  let initialMaxRuns = $state<number | null>(null);

  function resetPasswordForm() {
    currentPassword = '';
    newPassword = '';
    confirmPassword = '';
  }

  function close() {
    open = false;
    activeTab = 'general';
    resetPasswordForm();
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

  async function loadTotpStatus() {
    totpLoading = true;
    try {
      totpStatus = await api.auth.totpStatus();
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_2fa_load_error(), variant: 'error' });
    }
    totpLoading = false;
  }

  async function handleDisable2FA(e: Event) {
    e.preventDefault();
    disableLoading = true;
    try {
      await api.auth.totpDisable(disablePassword, disableCode);
      totpStatus = { enabled: false, recovery_codes_remaining: 0 };
      disablePassword = '';
      disableCode = '';
      showToast({ message: m.settings_2fa_disabled(), variant: 'success' });
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_2fa_disable_error(), variant: 'error' });
    }
    disableLoading = false;
  }

  function handleTotpEnabled() {
    loadTotpStatus();
  }

  $effect(() => {
    if (open) {
      loadSettings();
      loadTotpStatus();
    }
  });

  async function handlePasswordSubmit(e: Event) {
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

    passwordLoading = true;
    try {
      await api.auth.changePassword(currentPassword, newPassword);
      showToast({ message: m.settings_password_changed(), variant: 'success' });
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_password_error(), variant: 'error' });
    } finally {
      passwordLoading = false;
    }
  }

  async function handleLogsSave() {
    const value = maxRunsInput.trim() === '' ? null : parseInt(maxRunsInput, 10);

    if (value !== null && (isNaN(value) || value < 1)) {
      showToast({ message: m.settings_retention_invalid(), variant: 'error' });
      return;
    }

    logsLoading = true;
    try {
      await api.settings.update({ max_runs_per_job: value });
      maxRunsPerJob = value;
      initialMaxRuns = value;
      showToast({ message: m.settings_saved(), variant: 'success' });
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.settings_save_error(), variant: 'error' });
    } finally {
      logsLoading = false;
    }
  }

  function switchTab(tab: Tab) {
    activeTab = tab;
    if (tab === 'account') {
      resetPasswordForm();
    }
    if (tab === 'security') {
      disablePassword = '';
      disableCode = '';
    }
  }

  let hasLogsChanges = $derived(() => {
    const currentValue = maxRunsInput.trim() === '' ? null : parseInt(maxRunsInput, 10);
    if (isNaN(currentValue as number)) return false;
    return currentValue !== initialMaxRuns;
  });

  const tabs = [
    {
      id: 'general' as Tab,
      labelKey: () => m.settings_tab_general(),
      descriptionKey: () => m.settings_tab_general_desc(),
      icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z'
    },
    {
      id: 'account' as Tab,
      labelKey: () => m.settings_tab_account(),
      descriptionKey: () => m.settings_tab_account_desc(),
      icon: 'M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z'
    },
    {
      id: 'security' as Tab,
      labelKey: () => m.settings_tab_security(),
      descriptionKey: () => m.settings_tab_security_desc(),
      icon: 'M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z'
    },
    {
      id: 'credentials' as Tab,
      labelKey: () => m.settings_tab_credentials(),
      descriptionKey: () => m.settings_tab_credentials_desc(),
      icon: 'M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z'
    },
    {
      id: 'logs' as Tab,
      labelKey: () => m.settings_tab_history(),
      descriptionKey: () => m.settings_tab_history_desc(),
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
    <!-- Large dialog with sidebar -->
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-5xl h-[90vh] md:h-[85vh] flex flex-col overflow-hidden">
      <!-- Header -->
      <div class="px-6 py-5 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div>
          <h2 id="settings-title" class="text-xl font-semibold text-gray-900 dark:text-white">{m.settings_title()}</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">{m.settings_description()}</p>
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
        <nav class="md:w-56 lg:w-64 md:border-r border-b md:border-b-0 border-gray-200 dark:border-gray-700 flex-shrink-0 bg-gray-50/50 dark:bg-gray-900/30">
          <!-- Mobile: Horizontal scrollable tabs -->
          <div class="flex md:hidden p-2 gap-1.5 overflow-x-auto scroll-smooth snap-x snap-mandatory scrollbar-hide">
            {#each tabs as tab}
              <button
                onclick={() => switchTab(tab.id)}
                class="flex-shrink-0 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-all snap-start {activeTab === tab.id
                  ? 'bg-white dark:bg-gray-700 text-primary-600 dark:text-primary-400 shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-white/50 dark:hover:bg-gray-700/50'}"
                aria-selected={activeTab === tab.id}
                role="tab"
              >
                <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={tab.icon} />
                </svg>
                <span class="whitespace-nowrap">{tab.labelKey()}</span>
              </button>
            {/each}
          </div>

          <!-- Desktop: Vertical navigation -->
          <div class="hidden md:flex flex-col p-3 gap-1">
            {#each tabs as tab}
              <button
                onclick={() => switchTab(tab.id)}
                class="flex items-center gap-3 w-full px-3 py-2.5 rounded-xl text-left transition-all {activeTab === tab.id
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-white/50 dark:hover:bg-gray-700/50'}"
                aria-selected={activeTab === tab.id}
                role="tab"
              >
                <div class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 {activeTab === tab.id
                  ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-600 dark:text-primary-400'
                  : 'bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400'}">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={tab.icon} />
                  </svg>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="font-medium text-sm">{tab.labelKey()}</div>
                  <div class="text-xs text-gray-500 dark:text-gray-400 truncate">{tab.descriptionKey()}</div>
                </div>
              </button>
            {/each}
          </div>
        </nav>

        <!-- Content Panel - scrollable -->
        <div class="flex-1 overflow-y-auto">
          <div class="p-6 md:p-8 max-w-2xl">
            {#if activeTab === 'general'}
              <!-- General Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.settings_general_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.settings_general_description()}</p>
              </div>

              <div class="space-y-4">
                <!-- Job Execution Section -->
                <div class="space-y-3">
                  <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">{m.settings_job_execution()}</h4>

                  <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
                    <div class="relative flex items-center">
                      <input
                        type="checkbox"
                        checked={$preferencesStore.showLogViewerAfterManualRun}
                        onchange={(e) => preferencesStore.setShowLogViewerAfterManualRun(e.currentTarget.checked)}
                        class="peer sr-only"
                      />
                      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
                      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
                    </div>
                    <div class="flex-1 min-w-0">
                      <div class="font-medium text-gray-900 dark:text-white text-sm">{m.settings_show_log_viewer()}</div>
                      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.settings_show_log_viewer_desc()}</p>
                    </div>
                  </label>
                </div>

                <!-- Log Viewer Section -->
                <div class="space-y-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                  <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">{m.settings_log_viewer()}</h4>

                  <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
                    <div class="relative flex items-center">
                      <input
                        type="checkbox"
                        checked={$preferencesStore.autoShowLastPage}
                        onchange={(e) => preferencesStore.setAutoShowLastPage(e.currentTarget.checked)}
                        class="peer sr-only"
                      />
                      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
                      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
                    </div>
                    <div class="flex-1 min-w-0">
                      <div class="font-medium text-gray-900 dark:text-white text-sm">{m.settings_auto_show_last_page()}</div>
                      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.settings_auto_show_last_page_desc()}</p>
                    </div>
                  </label>

                </div>

                <!-- Language & Region Section -->
                <div class="space-y-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                  <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">{m.settings_language_title()}</h4>

                  <div class="p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl">
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 bg-primary-100 dark:bg-primary-900/40 rounded-xl flex items-center justify-center">
                          <svg class="w-5 h-5 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 5h12M9 3v2m1.048 9.5A18.022 18.022 0 016.412 9m6.088 9h7M11 21l5-10 5 10M12.751 5C11.783 10.77 8.07 15.61 3 18.129" />
                          </svg>
                        </div>
                        <div>
                          <div class="font-medium text-gray-900 dark:text-white text-sm">{m.settings_language_label()}</div>
                          <p class="text-xs text-gray-500 dark:text-gray-400">{m.settings_language_description()}</p>
                        </div>
                      </div>
                      <select
                        value={$languageStore}
                        onchange={(e) => languageStore.setLanguage(e.currentTarget.value as Locale)}
                        class="input w-auto min-w-[140px] text-sm"
                      >
                        {#each languageStore.available as lang}
                          <option value={lang}>{languageNames[lang]}</option>
                        {/each}
                      </select>
                    </div>
                  </div>
                </div>
              </div>

            {:else if activeTab === 'account'}
              <!-- Account Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.settings_password_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.settings_password_description()}</p>
              </div>

              <form onsubmit={handlePasswordSubmit} class="space-y-4">
                <div>
                    <label for="current-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.settings_current_password()}</label>
                    <input
                      id="current-password"
                      type="password"
                      required
                      bind:value={currentPassword}
                      class="input"
                      placeholder={m.settings_current_password_placeholder()}
                      autocomplete="current-password"
                    />
                  </div>

                  <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                      <label for="new-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.settings_new_password()}</label>
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
                      <label for="confirm-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.settings_confirm_password()}</label>
                      <input
                        id="confirm-password"
                        type="password"
                        required
                        bind:value={confirmPassword}
                        class="input"
                        placeholder={m.settings_confirm_password_placeholder()}
                        autocomplete="new-password"
                      />
                    </div>
                  </div>

                  <div class="flex justify-end pt-2">
                    <button type="submit" disabled={passwordLoading} class="btn btn-primary px-6 py-2">
                      {#if passwordLoading}
                        <span class="flex items-center gap-2">
                          <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                          </svg>
                          {m.settings_updating_password()}
                        </span>
                      {:else}
                        {m.settings_update_password()}
                      {/if}
                    </button>
                  </div>
                </form>

            {:else if activeTab === 'security'}
              <!-- Security Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.settings_2fa_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.settings_2fa_description()}</p>
              </div>

              {#if totpLoading}
                <div class="flex items-center justify-center py-12">
                  <svg class="animate-spin h-7 w-7 text-primary-600" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                  </svg>
                </div>
              {:else if totpStatus?.enabled}
                <!-- 2FA is enabled -->
                <div class="space-y-4">
                  <div class="flex items-center justify-between p-4 bg-green-50 dark:bg-green-900/20 rounded-xl border border-green-200 dark:border-green-800">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 bg-green-100 dark:bg-green-900/40 rounded-full flex items-center justify-center">
                        <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                        </svg>
                      </div>
                      <div>
                        <p class="font-medium text-green-800 dark:text-green-300">{m.settings_2fa_enabled()}</p>
                        <p class="text-sm text-green-600 dark:text-green-400">{m.settings_2fa_recovery_remaining({ count: totpStatus.recovery_codes_remaining })}</p>
                      </div>
                    </div>
                  </div>

                  {#if totpStatus.recovery_codes_remaining < 3}
                    <div class="p-4 bg-amber-50 dark:bg-amber-900/20 rounded-xl border border-amber-200 dark:border-amber-800">
                      <p class="text-sm text-amber-700 dark:text-amber-300">
                        {m.settings_2fa_low_codes()}
                      </p>
                    </div>
                  {/if}

                  <div class="p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl">
                    <div class="flex items-start gap-3 mb-4">
                      <div class="w-8 h-8 bg-red-100 dark:bg-red-900/40 rounded-lg flex items-center justify-center flex-shrink-0">
                        <svg class="w-4 h-4 text-red-600 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                        </svg>
                      </div>
                      <div>
                        <p class="text-sm font-medium text-gray-900 dark:text-white">{m.settings_2fa_disable_title()}</p>
                        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.settings_2fa_disable_description()}</p>
                      </div>
                    </div>

                    <form onsubmit={handleDisable2FA} class="space-y-4">
                      <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                          <label for="disable-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.auth_password()}</label>
                          <input
                            id="disable-password"
                            type="password"
                            bind:value={disablePassword}
                            class="input"
                            placeholder="Enter your password"
                            autocomplete="current-password"
                          />
                        </div>
                        <div>
                          <label for="disable-code" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.settings_2fa_authenticator_code()}</label>
                          <input
                            id="disable-code"
                            type="text"
                            inputmode="numeric"
                            pattern="[0-9]*"
                            maxlength="6"
                            bind:value={disableCode}
                            class="input font-mono tracking-widest"
                            placeholder="000000"
                            autocomplete="one-time-code"
                          />
                        </div>
                      </div>

                      <div class="flex justify-end pt-2">
                        <button
                          type="submit"
                          disabled={disableLoading || !disablePassword || disableCode.length !== 6}
                          class="btn btn-danger px-6 py-2 disabled:opacity-50"
                        >
                          {#if disableLoading}
                            <span class="flex items-center gap-2">
                              <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                              </svg>
                              {m.settings_2fa_disabling()}
                            </span>
                          {:else}
                            {m.settings_2fa_disable()}
                          {/if}
                        </button>
                      </div>
                    </form>
                  </div>
                </div>
              {:else}
                <!-- 2FA is not enabled - show setup -->
                <TotpSetup onEnabled={handleTotpEnabled} />
              {/if}

            {:else if activeTab === 'credentials'}
              <!-- Credentials Tab -->
              <CredentialsManager />

            {:else if activeTab === 'logs'}
              <!-- History Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.settings_retention_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.settings_retention_description()}</p>
              </div>

              <div class="space-y-4">
                <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl">
                  <span class="text-sm text-gray-600 dark:text-gray-400">{m.settings_retention_current()}</span>
                  <span class="text-sm font-medium text-gray-900 dark:text-white">
                    {#if maxRunsPerJob === null}
                      {m.settings_retention_unlimited()}
                    {:else}
                      {m.settings_retention_keep({ count: maxRunsPerJob })}
                    {/if}
                  </span>
                </div>

                <div>
                  <label for="max-runs" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.settings_retention_max_runs()}</label>
                  <div class="flex gap-3">
                    <div class="relative flex-1">
                      <input
                        id="max-runs"
                        type="text"
                        inputmode="numeric"
                        bind:value={maxRunsInput}
                        class="input pr-14"
                        placeholder={m.settings_retention_placeholder()}
                      />
                      <span class="absolute right-4 top-1/2 -translate-y-1/2 text-sm text-gray-400 dark:text-gray-500">{m.settings_retention_unit()}</span>
                    </div>
                    <button
                      onclick={handleLogsSave}
                      disabled={logsLoading || !hasLogsChanges()}
                      class="btn btn-primary px-6 py-2 disabled:opacity-50"
                    >
                      {#if logsLoading}
                        <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                      {:else}
                        {m.common_save()}
                      {/if}
                    </button>
                  </div>
                  <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">
                    {m.settings_retention_help()}
                  </p>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

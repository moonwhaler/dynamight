<script lang="ts">
  import { api } from '../lib/api';
  import PasswordStrength from './PasswordStrength.svelte';
  import { showToast } from './ui/Toast.svelte';
  import type { ImportPreview, ImportResult } from '../lib/types';
  import * as m from '$lib/paraglide/messages.js';

  let { open = $bindable(false) } = $props();

  type Tab = 'export' | 'import';
  let activeTab = $state<Tab>('export');

  // Export state
  let exportPassword = $state('');
  let exportPasswordConfirm = $state('');
  let exporting = $state(false);

  // Import state
  type ImportState = 'idle' | 'previewing' | 'previewed' | 'importing' | 'done';
  let importState = $state<ImportState>('idle');
  let importFile = $state<File | null>(null);
  let importPassword = $state('');
  let importStrategy = $state<'merge' | 'replace'>('merge');
  let preview = $state<ImportPreview | null>(null);
  let importResult = $state<ImportResult | null>(null);
  let importError = $state('');

  function close() {
    open = false;
    activeTab = 'export';
    resetExport();
    resetImport();
  }

  function resetExport() {
    exportPassword = '';
    exportPasswordConfirm = '';
    exporting = false;
  }

  function resetImport() {
    importState = 'idle';
    importFile = null;
    importPassword = '';
    importStrategy = 'merge';
    preview = null;
    importResult = null;
    importError = '';
  }

  function switchTab(tab: Tab) {
    activeTab = tab;
    if (tab === 'export') resetExport();
    if (tab === 'import') resetImport();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  async function handleExport() {
    if (exportPassword.length < 8) {
      showToast({ message: m.backup_password_too_short(), variant: 'error' });
      return;
    }
    if (exportPassword !== exportPasswordConfirm) {
      showToast({ message: m.backup_password_mismatch(), variant: 'error' });
      return;
    }

    exporting = true;
    try {
      const blob = await api.configBackup.export(exportPassword);
      const date = new Date().toISOString().split('T')[0];
      const filename = `dynamight-backup-${date}.dmbackup`;
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      showToast({ message: m.backup_export_success(), variant: 'success' });
      resetExport();
    } catch (err) {
      showToast({ message: err instanceof Error ? err.message : m.backup_export_error(), variant: 'error' });
    } finally {
      exporting = false;
    }
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files?.length) {
      importFile = input.files[0];
      importState = 'idle';
      preview = null;
      importResult = null;
      importError = '';
    }
  }

  async function handlePreview() {
    if (!importFile || importPassword.length < 8) {
      showToast({ message: m.backup_password_too_short(), variant: 'error' });
      return;
    }

    importState = 'previewing';
    importError = '';
    try {
      preview = await api.configBackup.preview(importFile, importPassword, importStrategy);
      importState = 'previewed';
    } catch (err) {
      importError = err instanceof Error ? err.message : m.backup_import_error();
      importState = 'idle';
    }
  }

  async function handleImport() {
    if (!importFile || importPassword.length < 8) return;

    importState = 'importing';
    importError = '';
    try {
      importResult = await api.configBackup.import(importFile, importPassword, importStrategy);
      importState = 'done';
      showToast({ message: m.backup_import_success(), variant: 'success' });

      // Reload the page after a short delay so the user sees the result,
      // since imported data (settings, jobs, credentials) needs a fresh load.
      // For replace mode, sessions are invalidated so a re-login is needed.
      setTimeout(() => window.location.reload(), 2000);
    } catch (err) {
      importError = err instanceof Error ? err.message : m.backup_import_error();
      importState = 'previewed';
    }
  }

  const tabs = [
    {
      id: 'export' as Tab,
      labelKey: () => m.backup_export_title(),
      descriptionKey: () => m.backup_export_description(),
      icon: 'M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4',
    },
    {
      id: 'import' as Tab,
      labelKey: () => m.backup_import_title(),
      descriptionKey: () => m.backup_import_description(),
      icon: 'M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12',
    },
  ];
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="backup-title"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-5xl h-[90vh] md:h-[85vh] flex flex-col overflow-hidden">
      <!-- Header -->
      <div class="px-6 py-5 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div>
          <h2 id="backup-title" class="text-xl font-semibold text-gray-900 dark:text-white">{m.backup_title()}</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">{m.backup_description()}</p>
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
          <!-- Mobile: Horizontal tabs -->
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
            {#if activeTab === 'export'}
              <!-- Export Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.backup_export_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.backup_export_description()}</p>
              </div>

              <div class="space-y-4">
                <div>
                  <label for="export-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.backup_password()}</label>
                  <input
                    id="export-password"
                    type="password"
                    bind:value={exportPassword}
                    class="input"
                    placeholder="Min. 8 characters"
                    autocomplete="new-password"
                  />
                  <PasswordStrength password={exportPassword} />
                </div>

                <div>
                  <label for="export-password-confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.backup_password_confirm()}</label>
                  <input
                    id="export-password-confirm"
                    type="password"
                    bind:value={exportPasswordConfirm}
                    class="input"
                    placeholder="Repeat password"
                    autocomplete="new-password"
                  />
                </div>

                <div class="flex justify-end pt-2">
                  <button
                    onclick={handleExport}
                    disabled={exporting || exportPassword.length < 8 || exportPassword !== exportPasswordConfirm}
                    class="btn btn-primary px-6 py-2.5 disabled:opacity-50"
                  >
                    {#if exporting}
                      <span class="flex items-center gap-2">
                        <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                        {m.backup_exporting()}
                      </span>
                    {:else}
                      {m.backup_export_button()}
                    {/if}
                  </button>
                </div>

                <!-- Info panels -->
                <div class="space-y-3 pt-4 border-t border-gray-200 dark:border-gray-700">
                  <div class="p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl">
                    <div class="flex items-start gap-3">
                      <div class="w-8 h-8 bg-gray-100 dark:bg-gray-700 rounded-lg flex items-center justify-center flex-shrink-0">
                        <svg class="w-4 h-4 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                      </div>
                      <div>
                        <p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">OpenSSL Compatible</p>
                        <p class="text-xs text-gray-500 dark:text-gray-400 font-mono break-all">{m.backup_password_hint()}</p>
                      </div>
                    </div>
                  </div>

                  <div class="p-4 bg-amber-50 dark:bg-amber-900/20 rounded-xl border border-amber-200 dark:border-amber-800">
                    <div class="flex items-start gap-3">
                      <div class="w-8 h-8 bg-amber-100 dark:bg-amber-900/40 rounded-lg flex items-center justify-center flex-shrink-0">
                        <svg class="w-4 h-4 text-amber-600 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                        </svg>
                      </div>
                      <p class="text-sm text-amber-700 dark:text-amber-300">{m.backup_oauth_warning()}</p>
                    </div>
                  </div>
                </div>
              </div>

            {:else if activeTab === 'import'}
              <!-- Import Tab -->
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.backup_import_title()}</h3>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.backup_import_description()}</p>
              </div>

              <div class="space-y-4">
                <!-- File picker -->
                <div>
                  <label for="import-file" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.backup_import_select_file()}</label>
                  <input
                    id="import-file"
                    type="file"
                    accept=".dmbackup"
                    onchange={handleFileSelect}
                    class="block w-full text-sm text-gray-500 dark:text-gray-400 file:mr-3 file:py-2 file:px-4 file:rounded-lg file:border-0 file:text-sm file:font-medium file:bg-primary-50 file:text-primary-700 dark:file:bg-primary-900/30 dark:file:text-primary-400 hover:file:bg-primary-100 dark:hover:file:bg-primary-900/50 file:cursor-pointer"
                  />
                </div>

                <!-- Password -->
                <div>
                  <label for="import-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">{m.backup_password()}</label>
                  <input
                    id="import-password"
                    type="password"
                    bind:value={importPassword}
                    class="input"
                    placeholder="Enter backup password"
                    autocomplete="off"
                  />
                </div>

                <!-- Strategy selector -->
                <div role="group" aria-labelledby="import-strategy-label">
                  <span id="import-strategy-label" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{m.backup_import_strategy()}</span>
                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <button
                      onclick={() => { importStrategy = 'merge'; importState = 'idle'; preview = null; }}
                      class="p-4 rounded-xl border-2 text-left transition-all {importStrategy === 'merge'
                        ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                        : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'}"
                    >
                      <div class="flex items-center gap-2 mb-1">
                        <svg class="w-4 h-4 {importStrategy === 'merge' ? 'text-primary-600 dark:text-primary-400' : 'text-gray-400'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 14v6m-3-3h6M6 10h2a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v2a2 2 0 002 2zm10 0h2a2 2 0 002-2V6a2 2 0 00-2-2h-2a2 2 0 00-2 2v2a2 2 0 002 2zM6 20h2a2 2 0 002-2v-2a2 2 0 00-2-2H6a2 2 0 00-2 2v2a2 2 0 002 2z" />
                        </svg>
                        <span class="font-medium text-sm text-gray-900 dark:text-white">{m.backup_import_merge()}</span>
                      </div>
                      <p class="text-xs text-gray-500 dark:text-gray-400">{m.backup_import_merge_desc()}</p>
                    </button>

                    <button
                      onclick={() => { importStrategy = 'replace'; importState = 'idle'; preview = null; }}
                      class="p-4 rounded-xl border-2 text-left transition-all {importStrategy === 'replace'
                        ? 'border-red-500 bg-red-50 dark:bg-red-900/20'
                        : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'}"
                    >
                      <div class="flex items-center gap-2 mb-1">
                        <svg class="w-4 h-4 {importStrategy === 'replace' ? 'text-red-600 dark:text-red-400' : 'text-gray-400'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                        </svg>
                        <span class="font-medium text-sm text-gray-900 dark:text-white">{m.backup_import_replace()}</span>
                      </div>
                      <p class="text-xs text-gray-500 dark:text-gray-400">{m.backup_import_replace_desc()}</p>
                      <p class="text-xs text-red-600 dark:text-red-400 mt-1.5 font-medium">{m.backup_import_replace_warning()}</p>
                    </button>
                  </div>
                </div>

                <!-- Error message -->
                {#if importError}
                  <div class="p-3 bg-red-50 dark:bg-red-900/20 rounded-xl border border-red-200 dark:border-red-800">
                    <p class="text-sm text-red-700 dark:text-red-300">{importError}</p>
                  </div>
                {/if}

                <!-- Preview button -->
                {#if importState === 'idle' || importState === 'previewing'}
                  <div class="flex justify-end pt-2">
                    <button
                      onclick={handlePreview}
                      disabled={!importFile || importPassword.length < 8 || importState === 'previewing'}
                      class="btn btn-primary px-6 py-2.5 disabled:opacity-50"
                    >
                      {#if importState === 'previewing'}
                        <span class="flex items-center gap-2">
                          <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                          </svg>
                          {m.backup_import_previewing()}
                        </span>
                      {:else}
                        {m.backup_import_preview_button()}
                      {/if}
                    </button>
                  </div>
                {/if}

                <!-- Preview results -->
                {#if preview && (importState === 'previewed' || importState === 'importing')}
                  <div class="space-y-4 pt-4 border-t border-gray-200 dark:border-gray-700">
                    <div class="p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl space-y-3">
                      <h4 class="font-medium text-sm text-gray-900 dark:text-white">{m.backup_preview_title()}</h4>

                      <div class="grid grid-cols-2 gap-3">
                        <div class="flex items-center gap-2.5 p-2.5 bg-white dark:bg-gray-800 rounded-lg">
                          <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                          </svg>
                          <span class="text-sm text-gray-700 dark:text-gray-300">{m.backup_preview_settings({ count: preview.settings_count })}</span>
                        </div>
                        <div class="flex items-center gap-2.5 p-2.5 bg-white dark:bg-gray-800 rounded-lg">
                          <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                          </svg>
                          <span class="text-sm text-gray-700 dark:text-gray-300">{m.backup_preview_credentials({ count: preview.credentials_count })}</span>
                        </div>
                        <div class="flex items-center gap-2.5 p-2.5 bg-white dark:bg-gray-800 rounded-lg">
                          <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
                          </svg>
                          <span class="text-sm text-gray-700 dark:text-gray-300">{m.backup_preview_jobs({ count: preview.jobs_count })}</span>
                        </div>
                        <div class="flex items-center gap-2.5 p-2.5 bg-white dark:bg-gray-800 rounded-lg">
                          <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                          </svg>
                          <span class="text-sm text-gray-700 dark:text-gray-300">{m.backup_preview_schedules({ count: preview.schedules_count })}</span>
                        </div>
                      </div>

                      <!-- Conflicts / warnings -->
                      {#if preview.conflicts.length > 0}
                        <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                          <p class="text-xs font-medium text-amber-700 dark:text-amber-400 mb-1.5">{m.backup_preview_conflicts()}</p>
                          <ul class="text-xs text-amber-600 dark:text-amber-300 space-y-1">
                            {#each preview.conflicts as conflict}
                              <li class="flex items-start gap-1.5">
                                <span class="mt-0.5 flex-shrink-0">&#x2022;</span>
                                <span>{conflict}</span>
                              </li>
                            {/each}
                          </ul>
                        </div>
                      {:else}
                        <p class="text-xs text-green-600 dark:text-green-400">{m.backup_preview_no_conflicts()}</p>
                      {/if}

                      {#if preview.has_oauth_credentials}
                        <div class="p-2.5 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
                          <p class="text-xs text-amber-700 dark:text-amber-300">{m.backup_preview_oauth_warning()}</p>
                        </div>
                      {/if}
                    </div>

                    <!-- Import button -->
                    <div class="flex justify-end">
                      <button
                        onclick={handleImport}
                        disabled={importState === 'importing'}
                        class="btn {importStrategy === 'replace' ? 'btn-danger' : 'btn-primary'} px-6 py-2.5 disabled:opacity-50"
                      >
                        {#if importState === 'importing'}
                          <span class="flex items-center gap-2">
                            <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                            </svg>
                            {m.backup_import_importing()}
                          </span>
                        {:else}
                          {m.backup_import_button()}
                        {/if}
                      </button>
                    </div>
                  </div>
                {/if}

                <!-- Import result -->
                {#if importResult && importState === 'done'}
                  <div class="p-4 bg-green-50 dark:bg-green-900/20 rounded-xl border border-green-200 dark:border-green-800 space-y-3">
                    <div class="flex items-center gap-3">
                      <div class="w-8 h-8 bg-green-100 dark:bg-green-900/40 rounded-full flex items-center justify-center flex-shrink-0">
                        <svg class="w-4 h-4 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                        </svg>
                      </div>
                      <h4 class="font-medium text-sm text-green-800 dark:text-green-300">{m.backup_result_title()}</h4>
                    </div>
                    <div class="text-sm text-green-700 dark:text-green-400 space-y-1 pl-11">
                      <p>{m.backup_result_settings({ count: importResult.settings_imported })}</p>
                      <p>{m.backup_result_credentials({ count: importResult.credentials_imported })}</p>
                      <p>{m.backup_result_jobs({ count: importResult.jobs_imported })}</p>
                      <p>{m.backup_result_schedules({ count: importResult.schedules_imported })}</p>
                    </div>

                    {#if importResult.warnings.length > 0}
                      <div class="pt-3 border-t border-green-200 dark:border-green-800">
                        <p class="text-xs font-medium text-amber-700 dark:text-amber-400 mb-1.5">{m.backup_result_warnings()}</p>
                        <ul class="text-xs text-amber-600 dark:text-amber-300 space-y-1">
                          {#each importResult.warnings as warning}
                            <li class="flex items-start gap-1.5">
                              <span class="mt-0.5 flex-shrink-0">&#x2022;</span>
                              <span>{warning}</span>
                            </li>
                          {/each}
                        </ul>
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

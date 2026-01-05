<script lang="ts">
  import { api } from '../../lib/api';
  import type { Credential, CredentialProviderType, CredentialUsage } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import CredentialEditModal from './CredentialEditModal.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // State
  let credentials = $state<Credential[]>([]);
  let credentialUsage = $state<Map<number, CredentialUsage>>(new Map());
  let loading = $state(true);
  let showEditModal = $state(false);
  let selectedCredential = $state<Credential | null>(null);
  let editMode = $state<'create' | 'edit'>('create');
  let selectedProviderType = $state<CredentialProviderType>('s3');
  let showProviderDropdown = $state(false);

  const providerTypes: { type: CredentialProviderType; label: string; icon: string }[] = [
    { type: 's3', label: 'S3 / Compatible', icon: 'M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z' },
    { type: 'sftp', label: 'SFTP', icon: 'M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01' },
    { type: 'webdav', label: 'WebDAV', icon: 'M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9' },
    { type: 'google_drive', label: 'Google Drive', icon: 'M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12' },
    { type: 'onedrive', label: 'OneDrive', icon: 'M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z' },
  ];

  function getProviderLabel(type: CredentialProviderType): string {
    return providerTypes.find(p => p.type === type)?.label || type;
  }

  function getProviderIcon(type: CredentialProviderType): string {
    return providerTypes.find(p => p.type === type)?.icon || '';
  }

  async function loadCredentials() {
    loading = true;
    try {
      credentials = await api.credentials.list();
      // Load usage for all credentials
      await Promise.all(credentials.map(loadUsageForCredential));
    } catch (e) {
      showToast({ message: m.error_failed_load(), variant: 'error' });
    } finally {
      loading = false;
    }
  }

  async function loadUsageForCredential(credential: Credential) {
    try {
      const usage = await api.credentials.getUsage(credential.id);
      credentialUsage.set(credential.id, usage);
      credentialUsage = new Map(credentialUsage);
    } catch {
      // Silently fail - usage info is not critical
    }
  }

  function handleAdd(type: CredentialProviderType) {
    selectedProviderType = type;
    selectedCredential = null;
    editMode = 'create';
    showEditModal = true;
    showProviderDropdown = false;
  }

  function handleEdit(credential: Credential) {
    selectedCredential = credential;
    selectedProviderType = credential.provider_type;
    editMode = 'edit';
    showEditModal = true;
  }

  async function handleDeleteClick(credential: Credential) {
    const usage = credentialUsage.get(credential.id);

    // Build confirmation message
    let message = String(m.credentials_delete_confirm_message({ name: credential.name }));
    if (usage && usage.count > 0) {
      message += `\n\n${String(m.credentials_delete_in_use_warning())}\n`;
      message += usage.jobs.map(j => `• ${j.name}`).join('\n');
      message += `\n\n${String(m.credentials_delete_in_use_note())}`;
    }

    const confirmed = await confirm({
      title: String(m.credentials_delete_confirm_title()),
      message,
      variant: 'danger',
      confirmText: String(m.common_delete()),
    });

    if (!confirmed) return;

    try {
      await api.credentials.delete(credential.id);
      showToast({ message: m.credentials_deleted(), variant: 'success' });
      await loadCredentials();
    } catch (e) {
      showToast({
        message: e instanceof Error ? e.message : m.error_credential_delete_failed(),
        variant: 'error',
      });
    }
  }

  function handleModalClose() {
    showEditModal = false;
    selectedCredential = null;
  }

  async function handleCredentialSaved() {
    await loadCredentials();
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  // Group credentials by provider type
  let groupedCredentials = $derived.by(() => {
    const groups = new Map<CredentialProviderType, Credential[]>();
    for (const cred of credentials) {
      const existing = groups.get(cred.provider_type) || [];
      existing.push(cred);
      groups.set(cred.provider_type, existing);
    }
    return groups;
  });

  // Load on mount
  $effect(() => {
    loadCredentials();
  });
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-start justify-between gap-4">
    <div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.credentials_manager_title()}</h3>
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.credentials_manager_description()}</p>
    </div>

    <!-- Add Credential Dropdown -->
    <div class="relative flex-shrink-0">
      <button
        type="button"
        class="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-primary-600 dark:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-lg transition-colors"
        onclick={() => (showProviderDropdown = !showProviderDropdown)}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        {m.credentials_add_new()}
        <svg class="w-3 h-3 opacity-60" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {#if showProviderDropdown}
        <div
          class="absolute right-0 mt-1 w-48 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 py-1 z-10"
          role="menu"
        >
          {#each providerTypes as provider}
            <button
              type="button"
              class="w-full px-3 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2.5"
              onclick={() => handleAdd(provider.type)}
              role="menuitem"
            >
              <svg class="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={provider.icon} />
              </svg>
              {provider.label}
            </button>
          {/each}
        </div>
        <!-- Backdrop to close dropdown -->
        <button
          type="button"
          class="fixed inset-0 z-0"
          onclick={() => (showProviderDropdown = false)}
          aria-label="Close menu"
        ></button>
      {/if}
    </div>
  </div>

  <!-- Loading State -->
  {#if loading}
    <div class="flex items-center justify-center py-12">
      <svg class="animate-spin h-7 w-7 text-primary-600" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
      </svg>
    </div>

  <!-- Empty State -->
  {:else if credentials.length === 0}
    <div class="text-center py-12 px-4">
      <div class="w-16 h-16 mx-auto mb-4 bg-gray-100 dark:bg-gray-800 rounded-full flex items-center justify-center">
        <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
        </svg>
      </div>
      <h4 class="text-lg font-medium text-gray-900 dark:text-white mb-1">{m.credentials_empty()}</h4>
      <p class="text-sm text-gray-500 dark:text-gray-400 max-w-sm mx-auto">{m.credentials_empty_description()}</p>
    </div>

  <!-- Credentials List -->
  {:else}
    <div class="space-y-6">
      {#each providerTypes as provider}
        {@const providerCredentials = groupedCredentials.get(provider.type) || []}
        {#if providerCredentials.length > 0}
          <div>
            <!-- Provider Section Header -->
            <div class="flex items-center gap-2 mb-3">
              <div class="w-8 h-8 bg-gray-100 dark:bg-gray-800 rounded-lg flex items-center justify-center">
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={provider.icon} />
                </svg>
              </div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">{provider.label}</h4>
              <span class="text-xs text-gray-400 dark:text-gray-500">({providerCredentials.length})</span>
            </div>

            <!-- Credential Cards -->
            <div class="space-y-2">
              {#each providerCredentials as credential}
                {@const usage = credentialUsage.get(credential.id)}
                <div class="bg-gray-50 dark:bg-gray-900/50 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
                  <div class="flex items-start justify-between gap-4">
                    <div class="flex-1 min-w-0">
                      <h5 class="font-medium text-gray-900 dark:text-white truncate">{credential.name}</h5>
                      <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                        Created {formatDate(credential.created_at)}
                        {#if credential.updated_at !== credential.created_at}
                          &bull; Updated {formatDate(credential.updated_at)}
                        {/if}
                      </p>
                      {#if usage}
                        <div class="mt-2">
                          {#if usage.count > 0}
                            <span class="inline-flex items-center gap-1 text-xs px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded-full">
                              <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                              </svg>
                              {m.credentials_used_by_jobs({ count: usage.count })}
                            </span>
                            {#if usage.jobs.length > 0}
                              <p class="text-xs text-gray-400 dark:text-gray-500 mt-1 truncate">
                                {usage.jobs.map(j => j.name).join(', ')}
                              </p>
                            {/if}
                          {:else}
                            <span class="text-xs text-gray-400 dark:text-gray-500">{m.credentials_not_in_use()}</span>
                          {/if}
                        </div>
                      {/if}
                    </div>

                    <!-- Actions -->
                    <div class="flex items-center gap-1">
                      <button
                        type="button"
                        class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700 rounded-lg transition-colors"
                        onclick={() => handleEdit(credential)}
                        aria-label={m.common_edit()}
                      >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                        </svg>
                      </button>
                      <button
                        type="button"
                        class="p-2 text-gray-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                        onclick={() => handleDeleteClick(credential)}
                        aria-label={m.common_delete()}
                      >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<!-- Edit/Create Modal -->
<CredentialEditModal
  bind:open={showEditModal}
  mode={editMode}
  credential={selectedCredential}
  providerType={selectedProviderType}
  onSave={handleCredentialSaved}
/>

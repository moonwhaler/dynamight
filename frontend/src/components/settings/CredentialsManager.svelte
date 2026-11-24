<script lang="ts">
  import { api } from '../../lib/api';
  import type { Credential, CredentialProviderType, CredentialUsage } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import CredentialEditModal from './CredentialEditModal.svelte';
  import SettingsSection from './SettingsSection.svelte';
  import Menu from '../ui/Menu.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // State
  let credentials = $state<Credential[]>([]);
  let credentialUsage = $state<Map<number, CredentialUsage>>(new Map());
  let loading = $state(true);
  let showEditModal = $state(false);
  let selectedCredential = $state<Credential | null>(null);
  let editMode = $state<'create' | 'edit'>('create');
  let selectedProviderType = $state<CredentialProviderType>('s3');

  const providerTypes: { type: CredentialProviderType; label: string; icon: IconName }[] = [
    { type: 's3', label: 'S3 / Compatible', icon: 'cloud' },
    { type: 'sftp', label: 'SFTP', icon: 'server' },
    { type: 'webdav', label: 'WebDAV', icon: 'globe' },
    { type: 'google_drive', label: 'Google Drive', icon: 'cloud' },
    { type: 'onedrive', label: 'OneDrive', icon: 'cloud' },
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

<SettingsSection title={m.credentials_manager_title()} description={m.credentials_manager_description()}>
  {#snippet actions()}
    <Menu
      label={m.credentials_add_new()}
      triggerClass="btn btn-secondary btn-sm"
      items={providerTypes.map((p) => ({ label: p.label, icon: p.icon, onclick: () => handleAdd(p.type) }))}
    >
      {#snippet trigger()}
        <Icon name="plus" class="w-4 h-4" />
        <span class="hidden sm:inline">{m.credentials_add_new()}</span>
      {/snippet}
    </Menu>
  {/snippet}

  {#if loading}
    <div class="flex justify-center py-10 text-primary-600"><Spinner size="w-6 h-6" /></div>
  {:else if credentials.length === 0}
    <EmptyState icon="key" title={m.credentials_empty()} text={m.credentials_empty_description()} compact />
  {:else}
    <div class="space-y-6">
      {#each providerTypes as provider (provider.type)}
        {@const providerCredentials = groupedCredentials.get(provider.type) || []}
        {#if providerCredentials.length > 0}
          <div>
            <p class="eyebrow mb-2 flex items-center gap-2">
              <Icon name={provider.icon} class="w-4 h-4" />
              {provider.label}
              <span class="tabular-nums">· {providerCredentials.length}</span>
            </p>
            <ul class="rows border border-gray-200 dark:border-gray-700 rounded-lg">
              {#each providerCredentials as credential (credential.id)}
                {@const usage = credentialUsage.get(credential.id)}
                <li class="flex items-start justify-between gap-3 px-4 py-3">
                  <div class="min-w-0">
                    <p class="font-medium text-gray-900 dark:text-white truncate">{credential.name}</p>
                    <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                      {m.credentials_created_on({ date: formatDate(credential.created_at) })}
                      {#if credential.updated_at !== credential.created_at}
                        · {m.credentials_updated_on({ date: formatDate(credential.updated_at) })}
                      {/if}
                    </p>
                    {#if usage}
                      <p class="text-xs mt-1 {usage.count > 0 ? 'text-gray-600 dark:text-gray-300' : 'text-gray-400 dark:text-gray-500'}">
                        {#if usage.count > 0}
                          {m.credentials_used_by_jobs({ count: usage.count })}{#if usage.jobs.length > 0}: {usage.jobs.map((j) => j.name).join(', ')}{/if}
                        {:else}
                          {m.credentials_not_in_use()}
                        {/if}
                      </p>
                    {/if}
                  </div>
                  <div class="-mt-1 -mr-2 shrink-0">
                  <Menu
                    label={m.common_actions()}
                    triggerClass="btn btn-ghost btn-icon btn-sm"
                    items={[
                      { label: m.common_edit(), icon: 'edit', onclick: () => handleEdit(credential) },
                      { label: m.common_delete(), icon: 'trash', danger: true, onclick: () => handleDeleteClick(credential) },
                    ]}
                  />
                  </div>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</SettingsSection>

<!-- Edit/Create Modal -->
<CredentialEditModal
  bind:open={showEditModal}
  mode={editMode}
  credential={selectedCredential}
  providerType={selectedProviderType}
  onSave={handleCredentialSaved}
/>

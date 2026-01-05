<script lang="ts">
  import { api } from '../../lib/api';
  import type { Credential, CredentialProviderType, CredentialData } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    providerType,
    selected = $bindable<number | null>(null),
    credentials = [],
    onCredentialsChange = () => {},
  }: {
    providerType: CredentialProviderType;
    selected: number | null;
    credentials: Credential[];
    onCredentialsChange?: () => void;
  } = $props();

  let showAddModal = $state(false);
  let saving = $state(false);

  // Form state for new credential
  let credentialName = $state('');

  // S3 fields
  let accessKeyId = $state('');
  let secretAccessKey = $state('');

  // SFTP fields
  let sftpPassword = $state('');
  let sftpPrivateKey = $state('');
  let sftpPassphrase = $state('');
  let usePrivateKey = $state(false);

  // WebDAV fields
  let webdavUsername = $state('');
  let webdavPassword = $state('');

  // OAuth fields (OneDrive, Google Drive)
  let oauthAccessToken = $state('');
  let oauthRefreshToken = $state('');

  function resetForm() {
    credentialName = '';
    accessKeyId = '';
    secretAccessKey = '';
    sftpPassword = '';
    sftpPrivateKey = '';
    sftpPassphrase = '';
    usePrivateKey = false;
    webdavUsername = '';
    webdavPassword = '';
    oauthAccessToken = '';
    oauthRefreshToken = '';
  }

  async function handleSave() {
    if (!credentialName.trim()) {
      showToast({ message: m.credentials_error_name_required(), variant: 'error' });
      return;
    }

    saving = true;

    try {
      let data: CredentialData;

      switch (providerType) {
        case 's3':
          if (!accessKeyId || !secretAccessKey) {
            showToast({ message: m.credentials_error_s3_incomplete(), variant: 'error' });
            saving = false;
            return;
          }
          data = {
            type: 's3',
            access_key_id: accessKeyId,
            secret_access_key: secretAccessKey,
          };
          break;

        case 'sftp':
          if (usePrivateKey) {
            if (!sftpPrivateKey) {
              showToast({ message: m.credentials_error_ssh_key_required(), variant: 'error' });
              saving = false;
              return;
            }
            data = {
              type: 'sftp',
              private_key: sftpPrivateKey,
              passphrase: sftpPassphrase || undefined,
            };
          } else {
            if (!sftpPassword) {
              showToast({ message: m.credentials_error_password_required(), variant: 'error' });
              saving = false;
              return;
            }
            data = {
              type: 'sftp',
              password: sftpPassword,
            };
          }
          break;

        case 'webdav':
          if (!webdavUsername || !webdavPassword) {
            showToast({ message: m.credentials_error_webdav_incomplete(), variant: 'error' });
            saving = false;
            return;
          }
          data = {
            type: 'webdav',
            username: webdavUsername,
            password: webdavPassword,
          };
          break;

        case 'onedrive':
        case 'google_drive':
          if (!oauthAccessToken || !oauthRefreshToken) {
            showToast({ message: m.credentials_error_oauth_incomplete(), variant: 'error' });
            saving = false;
            return;
          }
          data = {
            type: 'oauth',
            access_token: oauthAccessToken,
            refresh_token: oauthRefreshToken,
            expires_at: Math.floor(Date.now() / 1000) + 3600, // 1 hour from now
          };
          break;

        default:
          showToast({ message: m.credentials_error_unsupported_provider(), variant: 'error' });
          saving = false;
          return;
      }

      const credential = await api.credentials.create({
        name: credentialName,
        provider_type: providerType,
        data,
      });

      showToast({ message: m.credentials_saved(), variant: 'success' });
      selected = credential.id;
      showAddModal = false;
      resetForm();
      onCredentialsChange();
    } catch (e) {
      showToast({
        message: e instanceof Error ? e.message : m.credentials_save_failed(),
        variant: 'error',
      });
    } finally {
      saving = false;
    }
  }

  function getProviderLabel(type: CredentialProviderType): string {
    switch (type) {
      case 's3':
        return 'S3';
      case 'sftp':
        return 'SFTP';
      case 'webdav':
        return 'WebDAV';
      case 'google_drive':
        return 'Google Drive';
      case 'onedrive':
        return 'OneDrive';
      default:
        return type;
    }
  }

  let filteredCredentials = $derived(credentials.filter((c) => c.provider_type === providerType));
</script>

<div class="space-y-2">
  <label for="credential-select" class="label">
    {m.credentials_provider_label({ provider: getProviderLabel(providerType) })}
  </label>

  <div class="flex gap-2">
    <select id="credential-select" bind:value={selected} class="input flex-1">
      <option value={null}>{m.credentials_select_placeholder()}</option>
      {#each filteredCredentials as cred}
        <option value={cred.id}>{cred.name}</option>
      {/each}
    </select>

    <button
      type="button"
      class="btn btn-secondary whitespace-nowrap"
      onclick={() => (showAddModal = true)}
    >
      + {m.common_add()}
    </button>
  </div>

  {#if filteredCredentials.length === 0}
    <p class="text-sm text-gray-500 dark:text-gray-400">
      {m.credentials_none_configured({ provider: getProviderLabel(providerType) })}
    </p>
  {/if}
</div>

<!-- Add Credential Modal -->
{#if showAddModal}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    onclick={(e) => e.target === e.currentTarget && (showAddModal = false)}
    onkeydown={(e) => e.key === 'Escape' && (showAddModal = false)}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-xl w-full max-w-md">
      <div class="p-6 border-b border-gray-200 dark:border-gray-700">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
          {m.credentials_add_title({ provider: getProviderLabel(providerType) })}
        </h3>
      </div>

      <div class="p-6 space-y-4">
        <div>
          <label for="cred-name" class="label">{m.credentials_name_label()}</label>
          <input
            type="text"
            id="cred-name"
            bind:value={credentialName}
            placeholder={m.credentials_name_placeholder_dynamic({ provider: getProviderLabel(providerType) })}
            class="input"
          />
        </div>

        {#if providerType === 's3'}
          <div>
            <label for="access-key" class="label">{m.credentials_access_key()}</label>
            <input
              type="text"
              id="access-key"
              bind:value={accessKeyId}
              placeholder="AKIAIOSFODNN7EXAMPLE"
              class="input font-mono"
            />
          </div>
          <div>
            <label for="secret-key" class="label">{m.credentials_secret_key()}</label>
            <input
              type="password"
              id="secret-key"
              bind:value={secretAccessKey}
              placeholder="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
              class="input font-mono"
            />
          </div>
        {:else if providerType === 'sftp'}
          <div class="flex gap-4 mb-4">
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="auth-method"
                checked={!usePrivateKey}
                onchange={() => (usePrivateKey = false)}
                class="text-primary-600"
              />
              <span class="text-sm text-gray-700 dark:text-gray-300">{m.credentials_password()}</span>
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="auth-method"
                checked={usePrivateKey}
                onchange={() => (usePrivateKey = true)}
                class="text-primary-600"
              />
              <span class="text-sm text-gray-700 dark:text-gray-300">{m.credentials_ssh_key_label()}</span>
            </label>
          </div>

          {#if usePrivateKey}
            <div class="p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg mb-4">
              <p class="text-sm text-blue-800 dark:text-blue-300">
                <strong>{m.credentials_ssh_setup_label()}</strong> {m.credentials_ssh_setup_info({ path: '~/.ssh/authorized_keys' })}
              </p>
            </div>
            <div>
              <label for="private-key" class="label">{m.credentials_private_key_pem()}</label>
              <textarea
                id="private-key"
                bind:value={sftpPrivateKey}
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----&#10;..."
                rows="6"
                class="input font-mono text-xs"
              ></textarea>
            </div>
            <div>
              <label for="passphrase" class="label">{m.credentials_passphrase_optional()}</label>
              <input
                type="password"
                id="passphrase"
                bind:value={sftpPassphrase}
                placeholder={m.credentials_passphrase_placeholder()}
                class="input"
              />
            </div>
          {:else}
            <div>
              <label for="sftp-password" class="label">{m.credentials_password()}</label>
              <input
                type="password"
                id="sftp-password"
                bind:value={sftpPassword}
                placeholder={m.credentials_ssh_password_placeholder()}
                class="input"
              />
            </div>
          {/if}
        {:else if providerType === 'webdav'}
          <div>
            <label for="webdav-user" class="label">{m.credentials_username()}</label>
            <input
              type="text"
              id="webdav-user"
              bind:value={webdavUsername}
              placeholder={m.credentials_username_placeholder()}
              class="input"
            />
          </div>
          <div>
            <label for="webdav-pass" class="label">{m.credentials_password()}</label>
            <input
              type="password"
              id="webdav-pass"
              bind:value={webdavPassword}
              placeholder={m.credentials_password_placeholder()}
              class="input"
            />
          </div>
        {:else if providerType === 'onedrive' || providerType === 'google_drive'}
          <div class="p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg mb-4">
            <p class="text-sm text-amber-800 dark:text-amber-300">
              {#if providerType === 'onedrive'}
                {m.credentials_oauth_onedrive_help()}
              {:else}
                {m.credentials_oauth_google_help()}
              {/if}
            </p>
          </div>
          <div>
            <label for="access-token" class="label">{m.credentials_access_token()}</label>
            <textarea
              id="access-token"
              bind:value={oauthAccessToken}
              placeholder="eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiIs..."
              rows="3"
              class="input font-mono text-xs"
            ></textarea>
          </div>
          <div>
            <label for="refresh-token" class="label">{m.credentials_refresh_token()}</label>
            <textarea
              id="refresh-token"
              bind:value={oauthRefreshToken}
              placeholder="0.AAAA..."
              rows="2"
              class="input font-mono text-xs"
            ></textarea>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {m.credentials_refresh_token_help()}
            </p>
          </div>
        {/if}
      </div>

      <div class="p-6 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3">
        <button
          type="button"
          class="btn btn-secondary"
          onclick={() => {
            showAddModal = false;
            resetForm();
          }}
        >
          {m.common_cancel()}
        </button>
        <button type="button" class="btn btn-primary" disabled={saving} onclick={handleSave}>
          {saving ? m.credentials_saving() : m.credentials_save()}
        </button>
      </div>
    </div>
  </div>
{/if}

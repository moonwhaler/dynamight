<script lang="ts">
  import { api } from '../../lib/api';
  import type { Credential, CredentialProviderType, CredentialData } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';

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
      showToast({ message: 'Please enter a credential name', variant: 'error' });
      return;
    }

    saving = true;

    try {
      let data: CredentialData;

      switch (providerType) {
        case 's3':
          if (!accessKeyId || !secretAccessKey) {
            showToast({ message: 'Please fill in all S3 credential fields', variant: 'error' });
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
              showToast({ message: 'Please provide an SSH private key', variant: 'error' });
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
              showToast({ message: 'Please provide a password', variant: 'error' });
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
            showToast({ message: 'Please fill in all WebDAV credential fields', variant: 'error' });
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
            showToast({ message: 'Please fill in all OAuth credential fields', variant: 'error' });
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
          showToast({ message: 'Unsupported provider type', variant: 'error' });
          saving = false;
          return;
      }

      const credential = await api.credentials.create({
        name: credentialName,
        provider_type: providerType,
        data,
      });

      showToast({ message: 'Credential saved successfully', variant: 'success' });
      selected = credential.id;
      showAddModal = false;
      resetForm();
      onCredentialsChange();
    } catch (e) {
      showToast({
        message: e instanceof Error ? e.message : 'Failed to save credential',
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
    {getProviderLabel(providerType)} Credentials
  </label>

  <div class="flex gap-2">
    <select id="credential-select" bind:value={selected} class="input flex-1">
      <option value={null}>Select credentials...</option>
      {#each filteredCredentials as cred}
        <option value={cred.id}>{cred.name}</option>
      {/each}
    </select>

    <button
      type="button"
      class="btn btn-secondary whitespace-nowrap"
      onclick={() => (showAddModal = true)}
    >
      + Add
    </button>
  </div>

  {#if filteredCredentials.length === 0}
    <p class="text-sm text-gray-500 dark:text-gray-400">
      No {getProviderLabel(providerType)} credentials configured yet.
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
          Add {getProviderLabel(providerType)} Credentials
        </h3>
      </div>

      <div class="p-6 space-y-4">
        <div>
          <label for="cred-name" class="label">Credential Name</label>
          <input
            type="text"
            id="cred-name"
            bind:value={credentialName}
            placeholder="My {getProviderLabel(providerType)} Backup"
            class="input"
          />
        </div>

        {#if providerType === 's3'}
          <div>
            <label for="access-key" class="label">Access Key ID</label>
            <input
              type="text"
              id="access-key"
              bind:value={accessKeyId}
              placeholder="AKIAIOSFODNN7EXAMPLE"
              class="input font-mono"
            />
          </div>
          <div>
            <label for="secret-key" class="label">Secret Access Key</label>
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
              <span class="text-sm text-gray-700 dark:text-gray-300">Password</span>
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="auth-method"
                checked={usePrivateKey}
                onchange={() => (usePrivateKey = true)}
                class="text-primary-600"
              />
              <span class="text-sm text-gray-700 dark:text-gray-300">SSH Key</span>
            </label>
          </div>

          {#if usePrivateKey}
            <div class="p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg mb-4">
              <p class="text-sm text-blue-800 dark:text-blue-300">
                <strong>Setup:</strong> Add your public key to <code class="bg-blue-100 dark:bg-blue-800 px-1 rounded">~/.ssh/authorized_keys</code> on the server, then paste your private key below.
              </p>
            </div>
            <div>
              <label for="private-key" class="label">Private Key (PEM format)</label>
              <textarea
                id="private-key"
                bind:value={sftpPrivateKey}
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----&#10;..."
                rows="6"
                class="input font-mono text-xs"
              ></textarea>
            </div>
            <div>
              <label for="passphrase" class="label">Passphrase (optional)</label>
              <input
                type="password"
                id="passphrase"
                bind:value={sftpPassphrase}
                placeholder="Key passphrase"
                class="input"
              />
            </div>
          {:else}
            <div>
              <label for="sftp-password" class="label">Password</label>
              <input
                type="password"
                id="sftp-password"
                bind:value={sftpPassword}
                placeholder="SSH password"
                class="input"
              />
            </div>
          {/if}
        {:else if providerType === 'webdav'}
          <div>
            <label for="webdav-user" class="label">Username</label>
            <input
              type="text"
              id="webdav-user"
              bind:value={webdavUsername}
              placeholder="username"
              class="input"
            />
          </div>
          <div>
            <label for="webdav-pass" class="label">Password</label>
            <input
              type="password"
              id="webdav-pass"
              bind:value={webdavPassword}
              placeholder="password"
              class="input"
            />
          </div>
        {:else if providerType === 'onedrive' || providerType === 'google_drive'}
          <div class="p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg mb-4">
            <p class="text-sm text-amber-800 dark:text-amber-300">
              {#if providerType === 'onedrive'}
                To get OAuth tokens, you'll need to register an app in the Azure Portal
                and complete the OAuth flow to obtain access and refresh tokens.
              {:else}
                To get OAuth tokens, you'll need to set up OAuth credentials in the
                Google Cloud Console and complete the OAuth flow.
              {/if}
            </p>
          </div>
          <div>
            <label for="access-token" class="label">Access Token</label>
            <textarea
              id="access-token"
              bind:value={oauthAccessToken}
              placeholder="eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiIs..."
              rows="3"
              class="input font-mono text-xs"
            ></textarea>
          </div>
          <div>
            <label for="refresh-token" class="label">Refresh Token</label>
            <textarea
              id="refresh-token"
              bind:value={oauthRefreshToken}
              placeholder="0.AAAA..."
              rows="2"
              class="input font-mono text-xs"
            ></textarea>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              The refresh token is used to automatically renew access tokens when they expire.
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
          Cancel
        </button>
        <button type="button" class="btn btn-primary" disabled={saving} onclick={handleSave}>
          {saving ? 'Saving...' : 'Save Credentials'}
        </button>
      </div>
    </div>
  </div>
{/if}

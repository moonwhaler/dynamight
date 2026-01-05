<script lang="ts">
  import { api } from '../../lib/api';
  import type { Credential, CredentialProviderType, CredentialData, DestinationConfig } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';
  import TestConnection from '../jobs/TestConnection.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    open = $bindable(false),
    mode = 'create',
    credential = null,
    providerType,
    onSave = () => {},
  }: {
    open: boolean;
    mode: 'create' | 'edit';
    credential?: Credential | null;
    providerType: CredentialProviderType;
    onSave?: () => void;
  } = $props();

  let saving = $state(false);

  // Form state
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

  // Test connection fields (provider-specific destination config)
  let testBucket = $state('');
  let testRegion = $state('us-east-1');
  let testHost = $state('');
  let testPort = $state(22);
  let testUsername = $state('');
  let testUrl = $state('');
  let testFolderId = $state('');
  let testFolderPath = $state('/');

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
    testBucket = '';
    testRegion = 'us-east-1';
    testHost = '';
    testPort = 22;
    testUsername = '';
    testUrl = '';
    testFolderId = '';
    testFolderPath = '/';
  }

  $effect(() => {
    if (open && mode === 'edit' && credential) {
      credentialName = credential.name;
      // Reset sensitive fields - they need to be re-entered
      accessKeyId = '';
      secretAccessKey = '';
      sftpPassword = '';
      sftpPrivateKey = '';
      sftpPassphrase = '';
      webdavUsername = '';
      webdavPassword = '';
      oauthAccessToken = '';
      oauthRefreshToken = '';
    } else if (open && mode === 'create') {
      resetForm();
    }
  });

  function close() {
    open = false;
    resetForm();
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

  function getProviderLabel(type: CredentialProviderType): string {
    switch (type) {
      case 's3': return 'S3';
      case 'sftp': return 'SFTP';
      case 'webdav': return 'WebDAV';
      case 'google_drive': return 'Google Drive';
      case 'onedrive': return 'OneDrive';
      default: return type;
    }
  }

  async function handleSave() {
    if (!credentialName.trim()) {
      showToast({ message: m.credentials_error_name_required(), variant: 'error' });
      return;
    }

    saving = true;

    try {
      let data: CredentialData | undefined;
      const isEdit = mode === 'edit';

      // Build credential data based on provider type
      // In edit mode, only include data if fields were filled in
      switch (providerType) {
        case 's3':
          if (!isEdit || (accessKeyId && secretAccessKey)) {
            if (!accessKeyId || !secretAccessKey) {
              if (!isEdit) {
                showToast({ message: m.credentials_error_s3_incomplete(), variant: 'error' });
                saving = false;
                return;
              }
            } else {
              data = {
                type: 's3',
                access_key_id: accessKeyId,
                secret_access_key: secretAccessKey,
              };
            }
          }
          break;

        case 'sftp':
          if (usePrivateKey) {
            if (!isEdit || sftpPrivateKey) {
              if (!sftpPrivateKey && !isEdit) {
                showToast({ message: m.credentials_error_ssh_key_required(), variant: 'error' });
                saving = false;
                return;
              } else if (sftpPrivateKey) {
                data = {
                  type: 'sftp',
                  private_key: sftpPrivateKey,
                  passphrase: sftpPassphrase || undefined,
                };
              }
            }
          } else {
            if (!isEdit || sftpPassword) {
              if (!sftpPassword && !isEdit) {
                showToast({ message: m.credentials_error_password_required(), variant: 'error' });
                saving = false;
                return;
              } else if (sftpPassword) {
                data = {
                  type: 'sftp',
                  password: sftpPassword,
                };
              }
            }
          }
          break;

        case 'webdav':
          if (!isEdit || (webdavUsername && webdavPassword)) {
            if ((!webdavUsername || !webdavPassword) && !isEdit) {
              showToast({ message: m.credentials_error_webdav_incomplete(), variant: 'error' });
              saving = false;
              return;
            } else if (webdavUsername && webdavPassword) {
              data = {
                type: 'webdav',
                username: webdavUsername,
                password: webdavPassword,
              };
            }
          }
          break;

        case 'onedrive':
        case 'google_drive':
          if (!isEdit || (oauthAccessToken && oauthRefreshToken)) {
            if ((!oauthAccessToken || !oauthRefreshToken) && !isEdit) {
              showToast({ message: m.credentials_error_oauth_incomplete(), variant: 'error' });
              saving = false;
              return;
            } else if (oauthAccessToken && oauthRefreshToken) {
              data = {
                type: 'oauth',
                access_token: oauthAccessToken,
                refresh_token: oauthRefreshToken,
                expires_at: Math.floor(Date.now() / 1000) + 3600,
              };
            }
          }
          break;
      }

      if (mode === 'create') {
        if (!data) {
          showToast({ message: m.credentials_error_unsupported_provider(), variant: 'error' });
          saving = false;
          return;
        }
        await api.credentials.create({
          name: credentialName,
          provider_type: providerType,
          data,
        });
        showToast({ message: m.credentials_saved(), variant: 'success' });
      } else if (credential) {
        await api.credentials.update(credential.id, {
          name: credentialName,
          ...(data ? { data } : {}),
        });
        showToast({ message: m.credentials_updated(), variant: 'success' });
      }

      close();
      onSave();
    } catch (e) {
      showToast({
        message: e instanceof Error ? e.message : m.credentials_save_failed(),
        variant: 'error',
      });
    } finally {
      saving = false;
    }
  }

  // Build test destination config based on provider type
  let testDestination = $derived.by((): DestinationConfig | null => {
    switch (providerType) {
      case 's3':
        if (!testBucket || !testRegion) return null;
        return {
          type: 's3',
          bucket: testBucket,
          region: testRegion,
          prefix: '',
          endpoint: null,
          storage_class: null,
        };
      case 'sftp':
        if (!testHost || !testUsername) return null;
        return {
          type: 'sftp',
          host: testHost,
          port: testPort,
          username: testUsername,
          remote_path: '/',
          key_based_auth: usePrivateKey,
        };
      case 'webdav':
        if (!testUrl) return null;
        return {
          type: 'webdav',
          url: testUrl,
          remote_path: '/',
        };
      case 'google_drive':
        return {
          type: 'google_drive',
          folder_id: testFolderId,
          shared_drive_id: null,
        };
      case 'onedrive':
        return {
          type: 'onedrive',
          folder_path: testFolderPath,
          drive_id: null,
        };
      default:
        return null;
    }
  });

  // Build credential data from form fields for testing
  let testCredentialData = $derived.by((): CredentialData | undefined => {
    switch (providerType) {
      case 's3':
        if (!accessKeyId || !secretAccessKey) return undefined;
        return {
          type: 's3',
          access_key_id: accessKeyId,
          secret_access_key: secretAccessKey,
        };
      case 'sftp':
        if (usePrivateKey) {
          if (!sftpPrivateKey) return undefined;
          return {
            type: 'sftp',
            private_key: sftpPrivateKey,
            passphrase: sftpPassphrase || undefined,
          };
        } else {
          if (!sftpPassword) return undefined;
          return {
            type: 'sftp',
            password: sftpPassword,
          };
        }
      case 'webdav':
        if (!webdavUsername || !webdavPassword) return undefined;
        return {
          type: 'webdav',
          username: webdavUsername,
          password: webdavPassword,
        };
      case 'onedrive':
      case 'google_drive':
        if (!oauthAccessToken || !oauthRefreshToken) return undefined;
        return {
          type: 'oauth',
          access_token: oauthAccessToken,
          refresh_token: oauthRefreshToken,
          expires_at: Math.floor(Date.now() / 1000) + 3600,
        };
      default:
        return undefined;
    }
  });

  // Can test if we have destination config AND either existing credential or new credential data
  let canTest = $derived(
    testDestination !== null &&
      (mode === 'edit' ? (credential?.id || testCredentialData !== undefined) : testCredentialData !== undefined)
  );
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-[60] p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-xl w-full max-w-lg max-h-[90vh] flex flex-col">
      <div class="p-6 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
          {#if mode === 'create'}
            {m.credentials_add_title({ provider: getProviderLabel(providerType) })}
          {:else}
            {m.credentials_edit_title()}
          {/if}
        </h3>
      </div>

      <div class="p-6 space-y-4 overflow-y-auto flex-1">
        <!-- Credential Name -->
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

        {#if mode === 'edit'}
          <p class="text-sm text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-900/50 p-3 rounded-lg">
            {m.credentials_sensitive_unchanged()}
          </p>
        {/if}

        <!-- Provider-specific fields -->
        {#if providerType === 's3'}
          <div>
            <label for="access-key" class="label">{m.credentials_access_key()}</label>
            <input
              type="text"
              id="access-key"
              bind:value={accessKeyId}
              placeholder={mode === 'edit' ? '••••••••••••' : 'AKIAIOSFODNN7EXAMPLE'}
              class="input font-mono"
            />
          </div>
          <div>
            <label for="secret-key" class="label">{m.credentials_secret_key()}</label>
            <input
              type="password"
              id="secret-key"
              bind:value={secretAccessKey}
              placeholder={mode === 'edit' ? '••••••••••••' : 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY'}
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
                placeholder={mode === 'edit' ? '••••••••••••' : '-----BEGIN OPENSSH PRIVATE KEY-----\n...'}
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
                placeholder={mode === 'edit' ? '••••••••••••' : m.credentials_ssh_password_placeholder()}
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
              placeholder={mode === 'edit' ? '••••••••••••' : m.credentials_username_placeholder()}
              class="input"
            />
          </div>
          <div>
            <label for="webdav-pass" class="label">{m.credentials_password()}</label>
            <input
              type="password"
              id="webdav-pass"
              bind:value={webdavPassword}
              placeholder={mode === 'edit' ? '••••••••••••' : m.credentials_password_placeholder()}
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
              placeholder={mode === 'edit' ? '••••••••••••' : 'eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiIs...'}
              rows="3"
              class="input font-mono text-xs"
            ></textarea>
          </div>
          <div>
            <label for="refresh-token" class="label">{m.credentials_refresh_token()}</label>
            <textarea
              id="refresh-token"
              bind:value={oauthRefreshToken}
              placeholder={mode === 'edit' ? '••••••••••••' : '0.AAAA...'}
              rows="2"
              class="input font-mono text-xs"
            ></textarea>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {m.credentials_refresh_token_help()}
            </p>
          </div>
        {/if}

        <!-- Test Connection Section -->
        <div class="pt-4 border-t border-gray-200 dark:border-gray-700">
          <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{m.credentials_test_section()}</h4>
          <p class="text-xs text-gray-500 dark:text-gray-400 mb-3">{m.credentials_test_help()}</p>

          {#if providerType === 's3'}
            <div class="grid grid-cols-2 gap-3 mb-3">
              <div>
                <label for="test-bucket" class="label text-xs">{m.s3_bucket()}</label>
                <input type="text" id="test-bucket" bind:value={testBucket} placeholder="my-bucket" class="input text-sm" />
              </div>
              <div>
                <label for="test-region" class="label text-xs">{m.s3_region()}</label>
                <input type="text" id="test-region" bind:value={testRegion} placeholder="us-east-1" class="input text-sm" />
              </div>
            </div>
          {:else if providerType === 'sftp'}
            <div class="grid grid-cols-3 gap-3 mb-3">
              <div class="col-span-2">
                <label for="test-host" class="label text-xs">{m.sftp_host()}</label>
                <input type="text" id="test-host" bind:value={testHost} placeholder="sftp.example.com" class="input text-sm" />
              </div>
              <div>
                <label for="test-port" class="label text-xs">{m.sftp_port()}</label>
                <input type="number" id="test-port" bind:value={testPort} class="input text-sm" />
              </div>
            </div>
            <div class="mb-3">
              <label for="test-username" class="label text-xs">{m.sftp_username()}</label>
              <input type="text" id="test-username" bind:value={testUsername} placeholder="backup-user" class="input text-sm" />
            </div>
          {:else if providerType === 'webdav'}
            <div class="mb-3">
              <label for="test-url" class="label text-xs">{m.webdav_url()}</label>
              <input type="text" id="test-url" bind:value={testUrl} placeholder="https://cloud.example.com/remote.php/dav/files/user" class="input text-sm" />
            </div>
          {:else if providerType === 'google_drive'}
            <div class="mb-3">
              <label for="test-folder-id" class="label text-xs">{m.google_drive_folder_id()}</label>
              <input type="text" id="test-folder-id" bind:value={testFolderId} placeholder={m.google_drive_folder_id_placeholder_text()} class="input text-sm" />
            </div>
          {:else if providerType === 'onedrive'}
            <div class="mb-3">
              <label for="test-folder-path" class="label text-xs">{m.onedrive_folder_path()}</label>
              <input type="text" id="test-folder-path" bind:value={testFolderPath} placeholder="/Backups" class="input text-sm" />
            </div>
          {/if}

          {#if testDestination && canTest}
            <TestConnection
              destination={testDestination}
              credentialId={mode === 'edit' && credential && !testCredentialData ? credential.id : null}
              credentialData={testCredentialData}
            />
          {:else if testDestination && !canTest}
            <p class="text-xs text-gray-400 dark:text-gray-500 italic">{m.credentials_test_fill_credentials()}</p>
          {:else}
            <p class="text-xs text-gray-400 dark:text-gray-500 italic">{m.credentials_test_missing_config()}</p>
          {/if}
        </div>
      </div>

      <div class="p-6 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-3 flex-shrink-0">
        <button type="button" class="btn btn-secondary" onclick={close}>
          {m.common_cancel()}
        </button>
        <button type="button" class="btn btn-primary" disabled={saving} onclick={handleSave}>
          {saving ? m.credentials_saving() : m.credentials_save()}
        </button>
      </div>
    </div>
  </div>
{/if}

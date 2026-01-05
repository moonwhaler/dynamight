<script lang="ts">
  import type { GoogleDriveDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

  let {
    config = $bindable<GoogleDriveDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
  }: {
    config: GoogleDriveDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
  } = $props();
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="google_drive"
    bind:selected={credentialId}
    {credentials}
  />

  <div>
    <label for="folder-id" class="label">
      Folder ID
      <HelpTooltip text="The ID of the Google Drive folder where backups will be stored. Leave empty to use the root of your Drive. You can find the folder ID in the URL when viewing the folder in Google Drive." />
    </label>
    <input
      type="text"
      id="folder-id"
      bind:value={config.folder_id}
      placeholder="Leave empty for root folder"
      class="input font-mono"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.folder_id}
        Files will be synced to folder: {config.folder_id}
      {:else}
        Files will be synced to the root of your Google Drive
      {/if}
    </p>
  </div>

  <div>
    <label for="shared-drive-id" class="label">
      Shared Drive ID (Optional)
      <HelpTooltip text="Leave empty to use your personal 'My Drive'. Specify a Shared Drive ID to sync to a team drive." />
    </label>
    <input
      type="text"
      id="shared-drive-id"
      bind:value={config.shared_drive_id}
      placeholder="Leave empty for My Drive"
      class="input font-mono"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.shared_drive_id}
        Using Shared Drive: {config.shared_drive_id}
      {:else}
        Using your personal My Drive
      {/if}
    </p>
  </div>

  <div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
    <h3 class="font-medium text-blue-800 dark:text-blue-300 mb-2">Google Drive Setup</h3>
    <p class="text-sm text-blue-700 dark:text-blue-400 mb-2">
      Google Drive uses OAuth authentication. You'll need to:
    </p>
    <ol class="text-sm text-blue-700 dark:text-blue-400 list-decimal list-inside space-y-1">
      <li>Create a project in the Google Cloud Console</li>
      <li>Enable the Google Drive API</li>
      <li>Create OAuth 2.0 credentials</li>
      <li>Complete the OAuth flow to obtain access and refresh tokens</li>
    </ol>
  </div>
</div>

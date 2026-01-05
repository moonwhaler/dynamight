<script lang="ts">
  import type { OneDriveDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

  let {
    config = $bindable<OneDriveDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
  }: {
    config: OneDriveDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
  } = $props();
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="onedrive"
    bind:selected={credentialId}
    {credentials}
  />

  <div>
    <label for="folder-path" class="label">
      Folder Path
      <HelpTooltip text="The folder path in your OneDrive where backups will be stored. Use forward slashes, e.g., /Backups/MyServer" />
    </label>
    <input
      type="text"
      id="folder-path"
      bind:value={config.folder_path}
      placeholder="/Backups"
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Files will be synced to: OneDrive{config.folder_path}
    </p>
  </div>

  <div>
    <label for="drive-id" class="label">
      Drive ID (Optional)
      <HelpTooltip text="Leave empty to use your personal OneDrive. Specify a drive ID to use a shared drive or SharePoint document library." />
    </label>
    <input
      type="text"
      id="drive-id"
      bind:value={config.drive_id}
      placeholder="Leave empty for personal OneDrive"
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.drive_id}
        Using shared drive: {config.drive_id}
      {:else}
        Using your personal OneDrive
      {/if}
    </p>
  </div>

  <div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
    <h3 class="font-medium text-blue-800 dark:text-blue-300 mb-2">OneDrive Setup</h3>
    <p class="text-sm text-blue-700 dark:text-blue-400">
      OneDrive uses OAuth authentication. You'll need to add your Microsoft account credentials
      using the "Add Credentials" button above. The credentials will be securely stored and used
      to access your OneDrive.
    </p>
  </div>
</div>

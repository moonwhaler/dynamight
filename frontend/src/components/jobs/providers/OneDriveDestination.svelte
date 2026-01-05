<script lang="ts">
  import type { OneDriveDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import TestConnection from '../TestConnection.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    config = $bindable<OneDriveDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
    onCredentialsChange = () => {},
  }: {
    config: OneDriveDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
    onCredentialsChange?: () => void;
  } = $props();
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="onedrive"
    bind:selected={credentialId}
    {credentials}
    {onCredentialsChange}
  />

  <div>
    <label for="folder-path" class="label">
      {m.onedrive_folder_path()}
      <HelpTooltip text={m.onedrive_folder_path_help()} />
    </label>
    <input
      type="text"
      id="folder-path"
      bind:value={config.folder_path}
      placeholder={m.onedrive_folder_path_placeholder()}
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {m.onedrive_preview({ path: config.folder_path || '' })}
    </p>
  </div>

  <div>
    <label for="drive-id" class="label">
      {m.onedrive_drive_id()} ({m.common_optional()})
      <HelpTooltip text={m.onedrive_drive_id_help()} />
    </label>
    <input
      type="text"
      id="drive-id"
      bind:value={config.drive_id}
      placeholder={m.onedrive_drive_id_placeholder()}
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.drive_id}
        {m.onedrive_using_shared({ id: config.drive_id })}
      {:else}
        {m.onedrive_using_personal()}
      {/if}
    </p>
  </div>

  <div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
    <h3 class="font-medium text-blue-800 dark:text-blue-300 mb-2">{m.onedrive_setup_title()}</h3>
    <p class="text-sm text-blue-700 dark:text-blue-400">
      {m.onedrive_setup_desc()}
    </p>
  </div>

  <TestConnection
    destination={config}
    {credentialId}
    disabled={!config.folder_path || !credentialId}
  />
</div>

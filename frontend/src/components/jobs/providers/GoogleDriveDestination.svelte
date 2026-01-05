<script lang="ts">
  import type { GoogleDriveDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

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
      {m.google_drive_folder_id()}
      <HelpTooltip text={m.google_drive_folder_id_help()} />
    </label>
    <input
      type="text"
      id="folder-id"
      bind:value={config.folder_id}
      placeholder={m.google_drive_folder_id_placeholder_text()}
      class="input font-mono"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.folder_id}
        {m.google_drive_preview_folder({ id: config.folder_id })}
      {:else}
        {m.google_drive_preview_root()}
      {/if}
    </p>
  </div>

  <div>
    <label for="shared-drive-id" class="label">
      {m.google_drive_shared_drive()} ({m.common_optional()})
      <HelpTooltip text={m.google_drive_shared_drive_help()} />
    </label>
    <input
      type="text"
      id="shared-drive-id"
      bind:value={config.shared_drive_id}
      placeholder={m.google_drive_shared_drive_placeholder()}
      class="input font-mono"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if config.shared_drive_id}
        {m.google_drive_using_shared_drive({ id: config.shared_drive_id })}
      {:else}
        {m.google_drive_using_my_drive()}
      {/if}
    </p>
  </div>

  <div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
    <h3 class="font-medium text-blue-800 dark:text-blue-300 mb-2">{m.google_drive_setup_title()}</h3>
    <p class="text-sm text-blue-700 dark:text-blue-400 mb-2">
      {m.google_drive_setup_desc()}
    </p>
    <ol class="text-sm text-blue-700 dark:text-blue-400 list-decimal list-inside space-y-1">
      <li>{m.google_drive_setup_step1()}</li>
      <li>{m.google_drive_setup_step2()}</li>
      <li>{m.google_drive_setup_step3()}</li>
      <li>{m.google_drive_setup_step4()}</li>
    </ol>
  </div>
</div>

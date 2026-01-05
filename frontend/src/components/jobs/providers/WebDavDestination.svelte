<script lang="ts">
  import type { WebDavDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

  let {
    config = $bindable<WebDavDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
  }: {
    config: WebDavDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
  } = $props();

  // Common WebDAV URL templates
  const urlTemplates = [
    { label: 'Custom URL', value: '' },
    { label: 'Nextcloud', value: 'https://your-server.com/remote.php/dav/files/username/' },
    { label: 'ownCloud', value: 'https://your-server.com/remote.php/webdav/' },
  ];
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="webdav"
    bind:selected={credentialId}
    {credentials}
  />

  <div>
    <label for="url" class="label">
      WebDAV Server URL
      <HelpTooltip text="The WebDAV endpoint URL. For Nextcloud, this is usually https://your-server.com/remote.php/dav/files/username/" />
    </label>
    <input
      type="url"
      id="url"
      bind:value={config.url}
      placeholder="https://nextcloud.example.com/remote.php/dav/files/user/"
      class="input"
    />
    <div class="mt-2 flex flex-wrap gap-2">
      {#each urlTemplates as template}
        {#if template.value}
          <button
            type="button"
            class="text-xs px-2 py-1 rounded-full bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600"
            onclick={() => (config.url = template.value)}
          >
            {template.label}
          </button>
        {/if}
      {/each}
    </div>
  </div>

  <div>
    <label for="remote-path" class="label">
      Remote Path
      <HelpTooltip text="The folder path within your WebDAV server where backups will be stored." />
    </label>
    <input
      type="text"
      id="remote-path"
      bind:value={config.remote_path}
      placeholder="/Backups"
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Files will be stored at: {config.url}{config.remote_path}
    </p>
  </div>
</div>

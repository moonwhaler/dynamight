<script lang="ts">
  import type { WebDavDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import TestConnection from '../TestConnection.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    config = $bindable<WebDavDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
    onCredentialsChange = () => {},
  }: {
    config: WebDavDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
    onCredentialsChange?: () => void;
  } = $props();

  // Common WebDAV URL templates
  const urlTemplates = [
    { labelKey: () => m.webdav_template_custom(), value: '' },
    { labelKey: () => m.webdav_template_nextcloud(), value: 'https://your-server.com/remote.php/dav/files/username/' },
    { labelKey: () => m.webdav_template_owncloud(), value: 'https://your-server.com/remote.php/webdav/' },
  ];
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="webdav"
    bind:selected={credentialId}
    {credentials}
    {onCredentialsChange}
  />

  <div>
    <label for="url" class="label">
      {m.webdav_url()}
      <HelpTooltip text={m.webdav_url_help()} />
    </label>
    <input
      type="url"
      id="url"
      bind:value={config.url}
      placeholder={m.webdav_url_placeholder()}
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
            {template.labelKey()}
          </button>
        {/if}
      {/each}
    </div>
  </div>

  <div>
    <label for="remote-path" class="label">
      {m.webdav_remote_path()}
      <HelpTooltip text={m.webdav_remote_path_help()} />
    </label>
    <input
      type="text"
      id="remote-path"
      bind:value={config.remote_path}
      placeholder={m.webdav_remote_path_placeholder()}
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {m.webdav_preview({ url: config.url || '', path: config.remote_path || '' })}
    </p>
  </div>

  <TestConnection
    destination={config}
    {credentialId}
    disabled={!config.url || !credentialId}
  />
</div>

<script lang="ts">
  import type { SftpDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import TestConnection from '../TestConnection.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    config = $bindable<SftpDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
    onCredentialsChange = () => {},
  }: {
    config: SftpDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
    onCredentialsChange?: () => void;
  } = $props();
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="sftp"
    bind:selected={credentialId}
    {credentials}
    {onCredentialsChange}
  />

  <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
    <div class="sm:col-span-2">
      <label for="host" class="label">
        {m.sftp_host()}
        <HelpTooltip text={m.sftp_host_help()} />
      </label>
      <input
        type="text"
        id="host"
        bind:value={config.host}
        placeholder={m.sftp_host_placeholder()}
        class="input"
      />
    </div>

    <div>
      <label for="port" class="label">
        {m.sftp_port()}
        <HelpTooltip text={m.sftp_port_help()} />
      </label>
      <input
        type="number"
        id="port"
        bind:value={config.port}
        min="1"
        max="65535"
        class="input"
      />
    </div>
  </div>

  <div>
    <label for="username" class="label">
      {m.sftp_username()}
      <HelpTooltip text={m.sftp_username_help()} />
    </label>
    <input
      type="text"
      id="username"
      bind:value={config.username}
      placeholder={m.sftp_username_placeholder()}
      class="input"
    />
  </div>

  <div>
    <label for="remote-path" class="label">
      {m.sftp_remote_path()}
      <HelpTooltip text={m.sftp_remote_path_help()} />
    </label>
    <input
      type="text"
      id="remote-path"
      bind:value={config.remote_path}
      placeholder={m.sftp_remote_path_placeholder()}
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {m.sftp_preview({ user: config.username || '', host: config.host || '', path: config.remote_path || '' })}
    </p>
  </div>

  <label
    class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
  >
    <div class="relative flex items-center">
      <input type="checkbox" bind:checked={config.key_based_auth} class="peer sr-only" />
      <div
        class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
      ></div>
      <div
        class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
      ></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.sftp_key_auth()}
        <HelpTooltip text={m.sftp_key_auth_help()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {config.key_based_auth
          ? m.sftp_key_auth_desc_key()
          : m.sftp_key_auth_desc_password()}
      </p>
    </div>
  </label>

  <TestConnection
    destination={config}
    {credentialId}
    disabled={!config.host || !config.username || !config.remote_path || !credentialId}
  />
</div>

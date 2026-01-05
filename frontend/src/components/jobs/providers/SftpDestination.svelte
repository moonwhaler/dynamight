<script lang="ts">
  import type { SftpDestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

  let {
    config = $bindable<SftpDestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
  }: {
    config: SftpDestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
  } = $props();
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="sftp"
    bind:selected={credentialId}
    {credentials}
  />

  <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
    <div class="sm:col-span-2">
      <label for="host" class="label">
        Host
        <HelpTooltip text="The hostname or IP address of the SSH/SFTP server." />
      </label>
      <input
        type="text"
        id="host"
        bind:value={config.host}
        placeholder="backup.example.com"
        class="input"
      />
    </div>

    <div>
      <label for="port" class="label">
        Port
        <HelpTooltip text="The SSH port number (default is 22)." />
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
      Username
      <HelpTooltip text="The SSH username to authenticate with." />
    </label>
    <input
      type="text"
      id="username"
      bind:value={config.username}
      placeholder="backup-user"
      class="input"
    />
  </div>

  <div>
    <label for="remote-path" class="label">
      Remote Path
      <HelpTooltip text="The directory path on the remote server where backups will be stored." />
    </label>
    <input
      type="text"
      id="remote-path"
      bind:value={config.remote_path}
      placeholder="/home/backup-user/backups"
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Files will be synced to: {config.username}@{config.host}:{config.remote_path}
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
        Use SSH Key Authentication
        <HelpTooltip text="Authenticate using an SSH key pair. First, add your PUBLIC key to ~/.ssh/authorized_keys on the server, then store your PRIVATE key in the credentials here. More secure than passwords." />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {config.key_based_auth
          ? 'Private key stored in credentials'
          : 'Password stored in credentials'}
      </p>
    </div>
  </label>
</div>

<script lang="ts">
  import type { DestinationType } from '../../lib/types';

  interface Provider {
    type: DestinationType;
    name: string;
    icon: string;
    description: string;
    available: boolean;
  }

  let {
    selected = $bindable<DestinationType>('local'),
  }: {
    selected: DestinationType;
  } = $props();

  const providers: Provider[] = [
    {
      type: 'local',
      name: 'Local / USB',
      icon: 'hard-drive',
      description: 'Rsync to local or mounted drives',
      available: true,
    },
    {
      type: 's3',
      name: 'S3 / Compatible',
      icon: 'cloud',
      description: 'AWS S3, MinIO, Backblaze B2',
      available: true,
    },
    {
      type: 'sftp',
      name: 'SFTP',
      icon: 'server',
      description: 'Sync via SSH/SFTP',
      available: true,
    },
    {
      type: 'webdav',
      name: 'WebDAV',
      icon: 'globe',
      description: 'Nextcloud, ownCloud, etc.',
      available: true,
    },
    {
      type: 'google_drive',
      name: 'Google Drive',
      icon: 'cloud',
      description: 'Coming soon',
      available: false,
    },
    {
      type: 'onedrive',
      name: 'OneDrive',
      icon: 'cloud',
      description: 'Coming soon',
      available: false,
    },
  ];

  function selectProvider(type: DestinationType) {
    const provider = providers.find((p) => p.type === type);
    if (provider?.available) {
      selected = type;
    }
  }
</script>

<div class="space-y-3">
  <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300">Destination Type</h3>
  <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
    {#each providers as provider}
      <button
        type="button"
        disabled={!provider.available}
        onclick={() => selectProvider(provider.type)}
        class="relative flex flex-col items-center gap-2 p-4 rounded-xl border-2 transition-all text-left
          {selected === provider.type
          ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
          : provider.available
            ? 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600 bg-white dark:bg-gray-800'
            : 'border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 opacity-60 cursor-not-allowed'}"
      >
        <!-- Icon -->
        <div
          class="w-10 h-10 rounded-lg flex items-center justify-center
            {selected === provider.type
            ? 'bg-primary-100 dark:bg-primary-800 text-primary-600 dark:text-primary-400'
            : 'bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400'}"
        >
          {#if provider.icon === 'hard-drive'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
              />
            </svg>
          {:else if provider.icon === 'cloud'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z"
              />
            </svg>
          {:else if provider.icon === 'server'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"
              />
            </svg>
          {:else if provider.icon === 'globe'}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"
              />
            </svg>
          {/if}
        </div>

        <!-- Name and description -->
        <div class="text-center">
          <span class="block text-sm font-medium text-gray-900 dark:text-white">{provider.name}</span>
          <span class="block text-xs text-gray-500 dark:text-gray-400 mt-0.5">{provider.description}</span>
        </div>

        <!-- Selected indicator -->
        {#if selected === provider.type}
          <div class="absolute top-2 right-2">
            <svg class="w-5 h-5 text-primary-600 dark:text-primary-400" fill="currentColor" viewBox="0 0 20 20">
              <path
                fill-rule="evenodd"
                d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                clip-rule="evenodd"
              />
            </svg>
          </div>
        {/if}
      </button>
    {/each}
  </div>
</div>

<script lang="ts">
  import type { UsbDrive } from '$lib/types';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    drives: UsbDrive[];
    allowedPaths: string[];
    loadingDrives: boolean;
    onBrowseDrive: (drive: UsbDrive) => void;
    onBrowsePath: (path: string) => void;
    onMount: (drive: UsbDrive) => void;
    onUnmount: (drive: UsbDrive) => void;
    onRefresh: () => void;
    currentPath?: string;
  }

  let {
    drives,
    allowedPaths,
    loadingDrives,
    onBrowseDrive,
    onBrowsePath,
    onMount,
    onUnmount,
    onRefresh,
    currentPath = '',
  }: Props = $props();

  type TabType = 'paths' | 'drives';
  let activeTab = $state<TabType>('paths');

  // Set default tab based on available options
  $effect(() => {
    if (allowedPaths.length === 0 && drives.length > 0) {
      activeTab = 'drives';
    }
  });

  // Check if a path is currently being browsed
  function isPathActive(path: string): boolean {
    return currentPath.startsWith(path);
  }

  // Check if a drive's mount point is currently being browsed
  function isDriveActive(drive: UsbDrive): boolean {
    return drive.mountpoint ? currentPath.startsWith(drive.mountpoint) : false;
  }

  // Get the last segment of a path for display
  function getPathLabel(path: string): string {
    const segments = path.split('/').filter(Boolean);
    return segments[segments.length - 1] || path;
  }

  // Count mounted drives
  function getMountedCount(): number {
    return drives.filter(d => d.mountpoint).length;
  }
</script>

<div class="space-y-3">
  <!-- Segmented Tab Control -->
  <div class="flex items-center gap-2">
    <div class="inline-flex rounded-lg bg-gray-100 dark:bg-gray-800 p-1">
      {#if allowedPaths.length > 0}
        <button
          type="button"
          onclick={() => activeTab = 'paths'}
          class="flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-all
            {activeTab === 'paths'
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
              : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'}"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
          {m.filebrowser_local_paths()}
          <span class="text-xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300">
            {allowedPaths.length}
          </span>
        </button>
      {/if}
      <button
        type="button"
        onclick={() => activeTab = 'drives'}
        class="flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-all
          {activeTab === 'drives'
            ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
            : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'}"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
        </svg>
        {m.filebrowser_usb_drives()}
        {#if drives.length > 0}
          <span class="text-xs px-1.5 py-0.5 rounded-full {getMountedCount() > 0 ? 'bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-400' : 'bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300'}">
            {getMountedCount()}/{drives.length}
          </span>
        {/if}
      </button>
    </div>

    <!-- Refresh button for drives tab -->
    {#if activeTab === 'drives'}
      <button
        type="button"
        onclick={onRefresh}
        disabled={loadingDrives}
        class="p-1.5 rounded-lg text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-600 dark:hover:text-gray-300 disabled:opacity-50 transition-colors"
        title={m.filebrowser_refresh_drives()}
      >
        <svg class="w-4 h-4 {loadingDrives ? 'animate-spin' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>
    {/if}
  </div>

  <!-- Tab Content -->
  <div class="min-h-[60px]">
    <!-- Local Paths Tab -->
    {#if activeTab === 'paths' && allowedPaths.length > 0}
      <div class="flex flex-wrap gap-2">
        {#each allowedPaths as path (path)}
          <button
            type="button"
            onclick={() => onBrowsePath(path)}
            class="group flex items-center gap-2 px-3 py-2 rounded-lg border transition-all
              {isPathActive(path)
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                : 'border-gray-200 dark:border-gray-700 hover:border-primary-300 dark:hover:border-primary-700 hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300'}"
          >
            <svg class="w-5 h-5 {isPathActive(path) ? 'text-primary-500' : 'text-yellow-500'}" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
            </svg>
            <div class="text-left">
              <div class="text-sm font-medium">{getPathLabel(path)}</div>
              <div class="text-xs text-gray-500 dark:text-gray-400 truncate max-w-48">{path}</div>
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- USB Drives Tab -->
    {#if activeTab === 'drives'}
      {#if drives.length === 0}
        <div class="flex items-center justify-center py-4 text-gray-500 dark:text-gray-400">
          <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
          <span class="text-sm">{m.filebrowser_no_drives()}</span>
        </div>
      {:else}
        <div class="flex flex-wrap gap-2">
          {#each drives as drive (drive.uuid)}
            <div
              class="flex items-center gap-3 px-3 py-2 rounded-lg border transition-all
                {isDriveActive(drive)
                  ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                  : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'}"
            >
              <!-- Drive icon with status -->
              <div class="relative">
                <div class="w-9 h-9 rounded-lg flex items-center justify-center {drive.mountpoint ? 'bg-green-100 dark:bg-green-900/30' : 'bg-gray-100 dark:bg-gray-700'}">
                  <svg class="w-5 h-5 {drive.mountpoint ? 'text-green-600 dark:text-green-400' : 'text-gray-400 dark:text-gray-500'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
                  </svg>
                </div>
                <!-- Status indicator dot -->
                <span class="absolute -top-0.5 -right-0.5 flex h-2.5 w-2.5">
                  {#if drive.mountpoint}
                    <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
                    <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-green-500"></span>
                  {:else}
                    <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-gray-400 dark:bg-gray-500"></span>
                  {/if}
                </span>
              </div>

              <!-- Drive info -->
              <div class="min-w-0">
                <div class="text-sm font-medium text-gray-900 dark:text-white truncate max-w-32">
                  {drive.label || drive.name}
                </div>
                <div class="text-xs text-gray-500 dark:text-gray-400">
                  {drive.size || ''}{drive.fstype ? ` • ${drive.fstype}` : ''}
                </div>
              </div>

              <!-- Action button -->
              {#if drive.mountpoint}
                <div class="flex items-center gap-1">
                  <button
                    type="button"
                    onclick={() => onBrowseDrive(drive)}
                    class="p-1.5 rounded-md hover:bg-primary-100 dark:hover:bg-primary-900/30 text-primary-600 dark:text-primary-400 transition-colors"
                    title={m.filebrowser_browse()}
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    onclick={() => onUnmount(drive)}
                    class="p-1.5 rounded-md hover:bg-red-100 dark:hover:bg-red-900/30 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                    title={m.filebrowser_unmount()}
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                    </svg>
                  </button>
                </div>
              {:else}
                <button
                  type="button"
                  onclick={() => onMount(drive)}
                  class="px-3 py-1 text-xs font-medium rounded-md bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 hover:bg-green-200 dark:hover:bg-green-900/50 transition-colors"
                >
                  {m.filebrowser_mount()}
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

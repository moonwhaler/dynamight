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

  // Check if a path is currently being browsed
  function isPathActive(path: string): boolean {
    return currentPath.startsWith(path);
  }

  // Check if a drive's mount point is currently being browsed
  function isDriveActive(drive: UsbDrive): boolean {
    return drive.mountpoint ? currentPath.startsWith(drive.mountpoint) : false;
  }
</script>

<div class="space-y-4">
  <!-- USB Drives Section -->
  <div>
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 flex items-center gap-2">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
        </svg>
        {m.filebrowser_usb_drives()}
      </h3>
      <button
        type="button"
        onclick={onRefresh}
        disabled={loadingDrives}
        class="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 disabled:opacity-50"
        title="Refresh drives"
      >
        <svg class="w-4 h-4 {loadingDrives ? 'animate-spin' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>
    </div>

    {#if drives.length === 0}
      <p class="text-sm text-gray-500 dark:text-gray-400 py-2">{m.filebrowser_no_drives()}</p>
    {:else}
      <div class="flex flex-wrap gap-2">
        {#each drives as drive (drive.uuid)}
          <div
            class="flex items-center gap-2 p-2 rounded-lg border transition-all {isDriveActive(drive) ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20' : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'}"
          >
            <!-- Drive icon -->
            <div class="w-8 h-8 rounded-lg flex items-center justify-center {drive.mountpoint ? 'bg-green-100 dark:bg-green-900/30' : 'bg-gray-100 dark:bg-gray-700'}">
              <svg class="w-4 h-4 {drive.mountpoint ? 'text-green-600 dark:text-green-400' : 'text-gray-500 dark:text-gray-400'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
              </svg>
            </div>

            <!-- Drive info -->
            <div class="flex flex-col min-w-0">
              <span class="text-sm font-medium text-gray-900 dark:text-white truncate">
                {drive.label || drive.name}
              </span>
              <span class="text-xs text-gray-500 dark:text-gray-400">
                {drive.size || ''} {drive.fstype ? `• ${drive.fstype}` : ''}
              </span>
            </div>

            <!-- Status badge -->
            {#if drive.mountpoint}
              <span class="badge badge-success text-xs">{m.filebrowser_mounted()}</span>
            {:else}
              <span class="badge bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 text-xs">{m.filebrowser_not_mounted()}</span>
            {/if}

            <!-- Actions -->
            <div class="flex items-center gap-1 ml-1">
              {#if drive.mountpoint}
                <button
                  type="button"
                  onclick={() => onBrowseDrive(drive)}
                  class="p-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-600 text-gray-600 dark:text-gray-300"
                  title={m.filebrowser_browse()}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                </button>
                <button
                  type="button"
                  onclick={() => onUnmount(drive)}
                  class="p-1.5 rounded hover:bg-red-100 dark:hover:bg-red-900/30 text-gray-600 dark:text-gray-300 hover:text-red-600 dark:hover:text-red-400"
                  title={m.filebrowser_unmount()}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                  </svg>
                </button>
              {:else}
                <button
                  type="button"
                  onclick={() => onMount(drive)}
                  class="p-1.5 rounded hover:bg-green-100 dark:hover:bg-green-900/30 text-gray-600 dark:text-gray-300 hover:text-green-600 dark:hover:text-green-400"
                  title={m.filebrowser_mount()}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                  </svg>
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Local Paths Section -->
  {#if allowedPaths.length > 0}
    <div>
      <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 flex items-center gap-2 mb-2">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
        </svg>
        {m.filebrowser_local_paths()}
      </h3>
      <div class="flex flex-wrap gap-2">
        {#each allowedPaths as path (path)}
          <button
            type="button"
            onclick={() => onBrowsePath(path)}
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg border transition-colors {isPathActive(path) ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300' : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600 text-gray-700 dark:text-gray-300'}"
          >
            <svg class="w-4 h-4 text-primary-500" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
            </svg>
            <span class="text-sm">{path}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

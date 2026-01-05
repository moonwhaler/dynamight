<script lang="ts">
  import type { LocalDestinationConfig, UsbDrive } from '../../../lib/types';
  import { api } from '../../../lib/api';
  import SinglePathSelector from '../SinglePathSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import BrowseModal from '../../filebrowser/BrowseModal.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    config = $bindable<LocalDestinationConfig>(),
    drives = [],
  }: {
    config: LocalDestinationConfig;
    drives: UsbDrive[];
  } = $props();

  // Browse modal state
  let showBrowseModal = $state(false);
  let browseLoading = $state(false);
  let browseError = $state<string | null>(null);
  let browseMountPoint = $state('');
  let wasMountedByBrowse = $state(false);

  // Get the selected drive
  let selectedDrive = $derived(drives.find(d => d.uuid === config.usb_uuid));
  let canBrowse = $derived(!!config.usb_uuid && !!selectedDrive);

  async function handleBrowse() {
    if (!selectedDrive) return;

    browseLoading = true;
    browseError = null;

    try {
      // Check if already mounted
      if (selectedDrive.mountpoint) {
        // Already mounted - browse directly
        browseMountPoint = selectedDrive.mountpoint;
        wasMountedByBrowse = false;
        showBrowseModal = true;
      } else {
        // Need to mount first - generate mount point if not set
        let mountPoint = config.mount_point;

        if (!mountPoint) {
          // Generate a mount point
          const result = await api.system.generateMountPoint(selectedDrive.uuid, selectedDrive.label ?? undefined);
          mountPoint = result.mount_point;
          config.mount_point = mountPoint;
        }

        // Mount the drive
        await api.system.mount(selectedDrive.uuid, mountPoint);
        browseMountPoint = mountPoint;
        wasMountedByBrowse = true;
        showBrowseModal = true;
      }
    } catch (e) {
      browseError = e instanceof Error ? e.message : m.error_generic();
    } finally {
      browseLoading = false;
    }
  }

  function handleSelectPath(path: string) {
    // Calculate the subdirectory relative to the mount point
    if (path.startsWith(browseMountPoint)) {
      const subdir = path.slice(browseMountPoint.length).replace(/^\/+/, '');
      config.backup_subdir = subdir;
    } else {
      // Fallback: use full path as subdir (shouldn't normally happen)
      config.backup_subdir = path;
    }
    showBrowseModal = false;
  }

  async function handleBrowseClose() {
    // Optionally unmount if we mounted for browsing
    // For now, we keep it mounted - user can manually unmount if needed
    showBrowseModal = false;
    browseMountPoint = '';
    wasMountedByBrowse = false;
  }
</script>

<div class="space-y-4">
  <div>
    <label for="usb" class="label">
      {m.local_usb_drive()} ({m.common_optional()})
      <HelpTooltip
        text={m.local_usb_drive_help()}
      />
    </label>
    <div class="flex gap-2">
      <select id="usb" bind:value={config.usb_uuid} class="input flex-1">
        <option value={null}>{m.local_no_usb_mount()}</option>
        {#each drives as drive}
          <option value={drive.uuid}>
            {drive.label || drive.name} ({drive.uuid.slice(0, 8)}...) - {drive.size}
            {#if drive.mountpoint}({m.filebrowser_mounted()}){/if}
          </option>
        {/each}
      </select>
      {#if canBrowse}
        <button
          type="button"
          onclick={handleBrowse}
          disabled={browseLoading}
          class="btn btn-secondary flex items-center gap-2"
          title={m.filebrowser_browse()}
        >
          {#if browseLoading}
            <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
          {:else}
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
            </svg>
          {/if}
          <span class="hidden sm:inline">{m.filebrowser_browse()}</span>
        </button>
      {/if}
    </div>
    {#if browseError}
      <p class="mt-1 text-sm text-red-500">{browseError}</p>
    {/if}
  </div>

  <div>
    <label for="mount" class="label">
      {m.local_mount_point()}
      <HelpTooltip
        text={m.local_mount_point_help()}
      />
    </label>
    <SinglePathSelector bind:path={config.mount_point} placeholder="/mnt/backup" />
  </div>

  <div class="space-y-3">
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input type="checkbox" bind:checked={config.auto_mount} class="peer sr-only" />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          {m.local_auto_mount()}
          <HelpTooltip text={m.local_auto_mount_help()} />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.local_auto_mount_desc()}
        </p>
      </div>
    </label>
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input type="checkbox" bind:checked={config.auto_unmount} class="peer sr-only" />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          {m.local_auto_unmount()}
          <HelpTooltip text={m.local_auto_unmount_help()} />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.local_auto_unmount_desc()}
        </p>
      </div>
    </label>
  </div>

  <div>
    <label for="subdir" class="label">
      {m.local_backup_subdir()}
      <HelpTooltip text={m.local_backup_subdir_help()} />
    </label>
    <input type="text" id="subdir" bind:value={config.backup_subdir} class="input" />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {m.local_preview({ path: `${config.mount_point}/${config.backup_subdir}/` })}
    </p>
  </div>
</div>

<!-- Browse Modal for selecting backup subdirectory -->
<BrowseModal
  bind:open={showBrowseModal}
  rootPath={browseMountPoint}
  onSelect={handleSelectPath}
  onClose={handleBrowseClose}
/>

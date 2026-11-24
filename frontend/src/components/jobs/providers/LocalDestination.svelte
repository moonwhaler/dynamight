<script lang="ts">
  import type { LocalDestinationConfig, UsbDrive } from '../../../lib/types';
  import { api } from '../../../lib/api';
  import { getDriveDisplayName } from '$lib/utils/driveUtils';
  import SinglePathSelector from '../SinglePathSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import BrowseModal from '../../filebrowser/BrowseModal.svelte';
  import Toggle from '../../ui/Toggle.svelte';
  import Spinner from '../../ui/Spinner.svelte';
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
      <select
        id="usb"
        bind:value={config.usb_uuid}
        onchange={() => {
          // Mounting only applies to a USB drive: on when one is chosen, off without
          config.auto_mount = !!config.usb_uuid;
          config.auto_unmount = !!config.usb_uuid;
        }}
        class="input flex-1"
      >
        <option value={null}>{m.local_no_usb_mount()}</option>
        {#each drives as drive}
          <option value={drive.uuid}>
            {getDriveDisplayName(drive)} ({drive.uuid.slice(0, 8)}...) - {drive.size}
            {#if drive.mountpoint}({m.filebrowser_mounted()}){/if}
          </option>
        {/each}
      </select>
      {#if canBrowse}
        <button
          type="button"
          onclick={handleBrowse}
          disabled={browseLoading}
          class="btn btn-secondary"
          title={m.filebrowser_browse()}
        >
          {#if browseLoading}
            <Spinner />
          {:else}
            {m.filebrowser_browse()}
          {/if}
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

  {#if config.usb_uuid}
    <div class="space-y-4 pl-0 sm:pl-4 sm:border-l-2 border-gray-200 dark:border-gray-700">
      <Toggle label={m.local_auto_mount()} description={m.local_auto_mount_desc()} bind:checked={config.auto_mount}>
        {#snippet extra()}<HelpTooltip text={m.local_auto_mount_help()} />{/snippet}
      </Toggle>
      <Toggle label={m.local_auto_unmount()} description={m.local_auto_unmount_desc()} bind:checked={config.auto_unmount}>
        {#snippet extra()}<HelpTooltip text={m.local_auto_unmount_help()} />{/snippet}
      </Toggle>
    </div>
  {/if}

  <div>
    <label for="subdir" class="label">
      {m.local_backup_subdir()}
      <HelpTooltip text={m.local_backup_subdir_help()} />
    </label>
    <input type="text" id="subdir" bind:value={config.backup_subdir} class="input" />
    <p class="hint font-mono break-all">
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

<script lang="ts">
  import type { LocalDestinationConfig, UsbDrive } from '../../../lib/types';
  import SinglePathSelector from '../SinglePathSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

  let {
    config = $bindable<LocalDestinationConfig>(),
    drives = [],
  }: {
    config: LocalDestinationConfig;
    drives: UsbDrive[];
  } = $props();
</script>

<div class="space-y-4">
  <div>
    <label for="usb" class="label">
      USB Drive (Optional)
      <HelpTooltip
        text="Select a USB drive to automatically mount before backup. The drive is identified by its unique UUID, so it will work regardless of which USB port you use."
      />
    </label>
    <select id="usb" bind:value={config.usb_uuid} class="input">
      <option value={null}>No USB mount</option>
      {#each drives as drive}
        <option value={drive.uuid}>
          {drive.label || drive.name} ({drive.uuid.slice(0, 8)}...) - {drive.size}
        </option>
      {/each}
    </select>
  </div>

  <div>
    <label for="mount" class="label">
      Mount Point
      <HelpTooltip
        text="The directory path where your backup destination will be accessible. For USB drives, this is where the drive gets mounted."
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
          Auto-mount before backup
          <HelpTooltip text="Automatically mount the selected USB drive before the backup starts." />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Mount the USB drive automatically when the job runs.
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
          Auto-unmount after backup
          <HelpTooltip text="Safely unmount the USB drive after backup completes." />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Safely unmount the drive when the backup completes.
        </p>
      </div>
    </label>
  </div>

  <div>
    <label for="subdir" class="label">
      Backup Subdirectory
      <HelpTooltip text="A subfolder within the mount point where backups will be stored." />
    </label>
    <input type="text" id="subdir" bind:value={config.backup_subdir} class="input" />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Files will be backed up to: {config.mount_point}/{config.backup_subdir}/
    </p>
  </div>
</div>

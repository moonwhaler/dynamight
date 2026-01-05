<script lang="ts">
  import type { LocalDestinationConfig, UsbDrive } from '../../../lib/types';
  import SinglePathSelector from '../SinglePathSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

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
      {m.local_usb_drive()} ({m.common_optional()})
      <HelpTooltip
        text={m.local_usb_drive_help()}
      />
    </label>
    <select id="usb" bind:value={config.usb_uuid} class="input">
      <option value={null}>{m.local_no_usb_mount()}</option>
      {#each drives as drive}
        <option value={drive.uuid}>
          {drive.label || drive.name} ({drive.uuid.slice(0, 8)}...) - {drive.size}
        </option>
      {/each}
    </select>
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

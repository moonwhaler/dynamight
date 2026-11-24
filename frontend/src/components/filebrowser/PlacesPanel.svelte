<script lang="ts">
  import type { FileDrive, FilePlaces } from '$lib/types';
  import { getDriveFullName } from '$lib/utils/driveUtils';
  import { formatBytes } from '$lib/utils/format';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    places,
    loading,
    activeRoot,
    trashActive,
    collapsed = false,
    busyDrive = null,
    onOpen,
    onTrash,
    onMount,
    onUnmount,
    onRefresh,
  }: {
    places: FilePlaces | null;
    loading: boolean;
    /** The place (path or mount point) that holds the open folder */
    activeRoot: string | null;
    trashActive: boolean;
    /** Icons only (narrow sidebar) */
    collapsed?: boolean;
    /** UUID of a drive being mounted or unmounted */
    busyDrive?: string | null;
    onOpen: (path: string) => void;
    onTrash: () => void;
    onMount: (drive: FileDrive) => void;
    onUnmount: (drive: FileDrive) => void;
    onRefresh: () => void;
  } = $props();

  function lastSegment(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path;
  }

  function usedPercent(total: number | null, free: number | null): number | null {
    if (!total || free === null) return null;
    return Math.round(((total - free) / total) * 100);
  }

  const rowBase =
    'group w-full flex items-center gap-3 rounded-lg text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500/60';
  const rowTone = (active: boolean) =>
    active
      ? 'bg-primary-50 dark:bg-primary-900/25 text-primary-800 dark:text-primary-200'
      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800';
</script>

{#snippet spaceBar(total: number | null, free: number | null)}
  {@const used = usedPercent(total, free)}
  {#if used !== null && !collapsed}
    <div class="mt-1.5 h-1 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden" aria-hidden="true">
      <div
        class="h-full rounded-full {used >= 90 ? 'bg-red-500' : used >= 75 ? 'bg-amber-500' : 'bg-primary-500'}"
        style="width: {used}%"
      ></div>
    </div>
    <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
      {m.filebrowser_free_space({ space: formatBytes(free) })}
    </div>
  {/if}
{/snippet}

<nav aria-label={m.filebrowser_places()} class="space-y-5">
  <section>
    {#if !collapsed}<h2 class="eyebrow px-3 mb-1.5">{m.filebrowser_places()}</h2>{/if}
    {#if loading && !places}
      <div class="space-y-2 px-3" aria-hidden="true">
        {#each [1, 2] as n (n)}<div class="h-10 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse"></div>{/each}
      </div>
    {:else if places && places.places.length === 0}
      {#if !collapsed}<p class="px-3 text-sm text-gray-500 dark:text-gray-400">{m.filebrowser_no_paths()}</p>{/if}
    {:else if places}
      <ul class="space-y-0.5">
        {#each places.places as place (place.path)}
          {@const active = !trashActive && activeRoot === place.path}
          <li>
            <button
              type="button"
              class="{rowBase} {rowTone(active)} {collapsed ? 'justify-center p-2.5' : 'px-3 py-2'}"
              aria-current={active ? 'location' : undefined}
              title={collapsed ? place.path : undefined}
              onclick={() => onOpen(place.path)}
            >
              <Icon name="folder" class="w-5 h-5 shrink-0 {active ? 'text-primary-600 dark:text-primary-400' : 'text-gray-400'}" />
              {#if !collapsed}
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-medium truncate" title={place.path}>{lastSegment(place.path)}</span>
                  {@render spaceBar(place.total, place.free)}
                </span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section>
    {#if !collapsed}
      <div class="flex items-center justify-between px-3 mb-1.5">
        <h2 class="eyebrow">{m.filebrowser_usb_drives()}</h2>
        <button
          type="button"
          class="btn btn-ghost btn-icon btn-sm -mr-2"
          onclick={onRefresh}
          disabled={loading}
          aria-label={m.filebrowser_refresh_drives()}
          title={m.filebrowser_refresh_drives()}
        >
          <Icon name="refresh" class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
        </button>
      </div>
    {/if}
    {#if places && places.drives.length === 0}
      {#if !collapsed}<p class="px-3 text-sm text-gray-500 dark:text-gray-400">{m.filebrowser_no_drives()}</p>{/if}
    {:else if places}
      <ul class="space-y-0.5">
        {#each places.drives as drive (drive.uuid)}
          {@const active = !trashActive && !!drive.mountpoint && activeRoot === drive.mountpoint}
          {@const busy = busyDrive === drive.uuid}
          <li class="relative">
            <button
              type="button"
              class="{rowBase} {rowTone(active)} {collapsed ? 'justify-center p-2.5' : 'pl-3 pr-10 py-2'}"
              aria-current={active ? 'location' : undefined}
              title={collapsed ? getDriveFullName(drive) : drive.mountpoint ?? undefined}
              disabled={busy}
              onclick={() => (drive.mountpoint ? onOpen(drive.mountpoint) : onMount(drive))}
            >
              <Icon
                name="drive"
                class="w-5 h-5 shrink-0 {busy ? 'animate-pulse' : ''} {drive.mountpoint
                  ? active
                    ? 'text-primary-600 dark:text-primary-400'
                    : 'text-gray-500 dark:text-gray-400'
                  : 'text-gray-300 dark:text-gray-600'}"
              />
              {#if !collapsed}
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-medium truncate {drive.mountpoint ? '' : 'text-gray-500 dark:text-gray-400'}">
                    {getDriveFullName(drive)}
                  </span>
                  {#if drive.mountpoint}
                    {@render spaceBar(drive.total, drive.free)}
                  {:else}
                    <span class="block text-xs text-gray-500 dark:text-gray-400">
                      {busy ? m.filebrowser_mounting() : m.filebrowser_not_mounted({ size: drive.size ?? '' })}
                    </span>
                  {/if}
                </span>
              {/if}
            </button>
            {#if drive.mountpoint && !collapsed}
              <button
                type="button"
                class="btn btn-ghost btn-icon btn-sm absolute right-1 top-1.5"
                disabled={busy}
                onclick={() => onUnmount(drive)}
                aria-label={m.filebrowser_eject({ name: getDriveFullName(drive) })}
                title={m.filebrowser_eject({ name: getDriveFullName(drive) })}
              >
                <Icon name="eject" class="w-4 h-4" />
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="border-t border-gray-200 dark:border-gray-800 pt-3">
    <button
      type="button"
      class="{rowBase} {rowTone(trashActive)} {collapsed ? 'justify-center p-2.5' : 'px-3 py-2'}"
      aria-current={trashActive ? 'page' : undefined}
      title={collapsed ? m.filebrowser_trash() : undefined}
      onclick={onTrash}
    >
      <Icon name="trash" class="w-5 h-5 shrink-0 {trashActive ? 'text-primary-600 dark:text-primary-400' : 'text-gray-400'}" />
      {#if !collapsed}
        <span class="flex-1 text-sm font-medium">{m.filebrowser_trash()}</span>
        {#if places && places.trash_items > 0}
          <span class="text-xs tabular-nums px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300">
            {places.trash_items}
          </span>
        {/if}
      {/if}
    </button>
  </section>
</nav>

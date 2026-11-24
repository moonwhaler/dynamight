<script lang="ts">
  import { onMount } from 'svelte';
  import { api, ApiError } from '$lib/api';
  import { authStore } from '$lib/stores/auth';
  import type { TrashItem, TrashListing, TrashRef } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import { formatModified, formatModifiedFull } from '$lib/files/fileDates';
  import { parentOf } from '$lib/files/browser.svelte';
  import { getLocale } from '$lib/paraglide/runtime.js';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from '../ui/Icon.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { onChanged, onOpenFolder }: { onChanged: () => void; onOpenFolder: (path: string) => void } = $props();

  const locale = getLocale();
  const now = new Date();

  let listing = $state<TrashListing | null>(null);
  let retentionDays = $state<number | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      listing = await api.files.listTrash();
    } catch (e) {
      error = e instanceof Error ? e.message : m.error_generic();
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    api.settings
      .get()
      .then((s) => (retentionDays = s.trash_retention_days))
      .catch(() => {});
  });

  function daysLeft(item: TrashItem): number | null {
    if (!retentionDays) return null;
    const expires = item.deleted_at + retentionDays * 86_400;
    return Math.max(0, Math.ceil((expires - Date.now() / 1000) / 86_400));
  }

  const ref = (item: TrashItem): TrashRef => ({ root: item.root, id: item.id });

  async function restore(items: TrashItem[], keepBoth = false) {
    busy = true;
    try {
      const result = await api.files.restore(items.map(ref), keepBoth);
      if (result.conflicts.length > 0 && !keepBoth) {
        const again = await confirm({
          title: m.filebrowser_restore_conflict_title(),
          message: m.filebrowser_restore_conflict_text({ count: result.conflicts.length }),
          confirmText: m.filebrowser_keep_both(),
          variant: 'info',
        });
        if (again) {
          const ids = new Set(result.conflicts.map((c) => c.id));
          await restore(items.filter((i) => ids.has(i.id)), true);
          return;
        }
      }
      if (result.restored.length === 1) {
        const path = result.restored[0].path;
        showToast({
          message: m.filebrowser_restored_one(),
          variant: 'success',
          action: { label: m.filebrowser_show_in_folder(), onclick: () => onOpenFolder(parentOf(path)) },
        });
      } else if (result.restored.length > 1) {
        showToast({ message: m.filebrowser_restored_many({ count: result.restored.length }), variant: 'success' });
      }
      if (result.failed.length > 0) showToast({ message: m.filebrowser_restore_failed(), variant: 'error' });
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      busy = false;
      await load();
      onChanged();
    }
  }

  // ── Delete for good (password check when needed) ──

  let purgeOpen = $state(false);
  let purgeBody = $state<{ items?: TrashRef[]; all?: boolean; roots?: string[] }>({});
  let purgeCount = $state(0);
  let needsPassword = $state(false);
  let password = $state('');
  let totp = $state('');
  let purgeError = $state<string | null>(null);
  const needs2FA = $derived($authStore.user?.totp_enabled ?? false);

  async function askPurge(body: typeof purgeBody, count: number) {
    purgeBody = body;
    purgeCount = count;
    password = '';
    totp = '';
    purgeError = null;
    needsPassword = !(await api.system.deleteStatus().then((s) => s.verified).catch(() => false));
    purgeOpen = true;
  }

  async function purge(e: Event) {
    e.preventDefault();
    purgeError = null;
    busy = true;
    try {
      if (needsPassword) {
        await api.system.verifyDeleteAccess(password, needs2FA ? totp : undefined);
        needsPassword = false;
      }
      const result = await api.files.purge(purgeBody);
      purgeOpen = false;
      showToast({ message: m.filebrowser_purged({ count: result.purged }), variant: 'success' });
      await load();
      onChanged();
    } catch (err) {
      if (err instanceof ApiError && err.code === 'DELETE_VERIFICATION_REQUIRED') needsPassword = true;
      purgeError = err instanceof Error ? err.message : m.error_delete_failed();
    } finally {
      busy = false;
    }
  }

  function menuFor(item: TrashItem): MenuItem[] {
    return [
      { label: m.filebrowser_restore(), icon: 'restore', onclick: () => restore([item]) },
      { label: m.filebrowser_show_original_folder(), icon: 'folder-open', onclick: () => onOpenFolder(parentOf(item.original_path)) },
      'separator',
      { label: m.filebrowser_delete_forever(), icon: 'trash', danger: true, onclick: () => askPurge({ items: [ref(item)] }, 1) },
    ];
  }

  const roots = $derived(listing?.roots.filter((r) => r.items > 0) ?? []);
  const emptyItems = $derived<MenuItem[]>(
    roots.map((r) => ({
      label: m.filebrowser_empty_trash_on({ place: r.anchor, count: r.items }),
      icon: 'trash',
      danger: true,
      onclick: () => askPurge({ all: true, roots: [r.root] }, r.items),
    })),
  );
</script>

<div class="flex flex-wrap items-center justify-between gap-3 px-4 py-3 border-b border-gray-200 dark:border-gray-700">
  <p class="text-sm text-gray-600 dark:text-gray-400">
    {retentionDays ? m.filebrowser_trash_hint_days({ days: retentionDays }) : m.filebrowser_trash_hint_keep()}
  </p>
  {#if roots.length === 1}
    <button type="button" class="btn btn-danger-ghost btn-sm" disabled={busy} onclick={() => askPurge({ all: true, roots: [roots[0].root] }, roots[0].items)}>
      <Icon name="trash" class="w-4 h-4" />{m.filebrowser_empty_trash()}
    </button>
  {:else if roots.length > 1}
    <Menu label={m.filebrowser_empty_trash()} items={emptyItems} triggerClass="btn btn-danger-ghost btn-sm">
      {#snippet trigger()}<Icon name="trash" class="w-4 h-4" />{m.filebrowser_empty_trash()}<Icon name="chevron-down" class="w-3.5 h-3.5" />{/snippet}
    </Menu>
  {/if}
</div>

{#if loading && !listing}
  <div class="py-16 flex justify-center" role="status" aria-label={m.filebrowser_loading()}>
    <Icon name="refresh" class="w-6 h-6 text-primary-500 animate-spin" />
  </div>
{:else if error}
  <div class="py-14 px-6 text-center" role="alert">
    <p class="text-sm text-gray-700 dark:text-gray-300">{error}</p>
    <button type="button" class="btn btn-secondary btn-sm mt-4" onclick={load}>{m.common_retry()}</button>
  </div>
{:else if listing && listing.items.length === 0}
  <div class="py-16 px-6 text-center text-sm text-gray-500 dark:text-gray-400">
    <Icon name="trash" class="w-8 h-8 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
    {m.filebrowser_trash_empty()}
  </div>
{:else if listing}
  <ul class="divide-y divide-gray-100 dark:divide-gray-800" aria-label={m.filebrowser_trash()}>
    {#each listing.items as item (item.root + item.id)}
      {@const left = daysLeft(item)}
      <li class="group flex items-center gap-3 px-4 py-2.5">
        <Icon name={item.is_dir ? 'folder' : 'file'} class="w-5 h-5 shrink-0 {item.is_dir ? 'text-primary-500' : 'text-gray-400'}" />
        <div class="min-w-0 flex-1">
          <div class="text-sm font-medium text-gray-900 dark:text-white truncate" title={item.name}>{item.name}</div>
          <div class="text-xs text-gray-500 dark:text-gray-400 truncate" title={item.original_path}>
            {parentOf(item.original_path)}
          </div>
        </div>
        <div class="hidden sm:block text-right text-xs text-gray-500 dark:text-gray-400 tabular-nums shrink-0">
          <div title={formatModifiedFull(item.deleted_at, locale)}>{m.filebrowser_deleted_on({ date: formatModified(item.deleted_at, now, locale) })}</div>
          {#if left !== null}<div>{m.filebrowser_removed_in({ days: left })}</div>{/if}
        </div>
        <div class="hidden md:block w-20 text-right text-sm text-gray-600 dark:text-gray-400 tabular-nums shrink-0">
          {item.size != null ? formatBytes(item.size) : ''}
        </div>
        <button type="button" class="btn btn-secondary btn-sm shrink-0" disabled={busy} onclick={() => restore([item])}>
          <Icon name="restore" class="w-4 h-4" /><span class="hidden sm:inline">{m.filebrowser_restore()}</span>
        </button>
        <Menu label={m.filebrowser_actions_for({ name: item.name })} items={menuFor(item)} triggerClass="btn btn-ghost btn-icon btn-sm" />
      </li>
    {/each}
  </ul>
{/if}

<Dialog bind:open={purgeOpen} title={m.filebrowser_delete_forever_title()} size="sm">
  <form id="purge-form" onsubmit={purge} class="space-y-4">
    <p class="text-sm text-gray-600 dark:text-gray-300">{m.filebrowser_purge_text({ count: purgeCount })}</p>
    {#if needsPassword}
      <div class="space-y-3 rounded-lg bg-gray-50 dark:bg-gray-800/60 p-3">
        <p class="text-xs text-gray-500 dark:text-gray-400">{m.filebrowser_delete_verify_hint()}</p>
        <div>
          <label for="purge-password" class="label">{m.filebrowser_delete_password()}</label>
          <input id="purge-password" type="password" class="input" bind:value={password} autocomplete="current-password" />
        </div>
        {#if needs2FA}
          <div>
            <label for="purge-totp" class="label">{m.filebrowser_delete_totp()}</label>
            <input id="purge-totp" class="input font-mono tracking-widest" bind:value={totp} inputmode="numeric" autocomplete="one-time-code" maxlength="8" />
          </div>
        {/if}
      </div>
    {/if}
    {#if purgeError}<p class="text-sm text-red-600 dark:text-red-400" role="alert">{purgeError}</p>{/if}
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-secondary" onclick={() => (purgeOpen = false)}>{m.common_cancel()}</button>
    <button type="submit" form="purge-form" class="btn btn-danger" disabled={busy}>{m.filebrowser_delete_forever()}</button>
  {/snippet}
</Dialog>

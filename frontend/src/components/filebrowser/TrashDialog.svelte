<script lang="ts" module>
  export interface DeleteTarget {
    path: string;
    name: string;
    is_dir: boolean;
  }
</script>

<script lang="ts">
  import { api } from '$lib/api';
  import { authStore } from '$lib/stores/auth';
  import type { TrashItem } from '$lib/types';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    open = $bindable(false),
    targets,
    onTrashed,
    onDeleted,
  }: {
    open?: boolean;
    targets: DeleteTarget[];
    /** Items now in the trash (for the Undo toast) */
    onTrashed: (items: TrashItem[]) => void;
    /** Paths removed for good */
    onDeleted: (paths: string[]) => void;
  } = $props();

  let permanent = $state(false);
  /** Items the trash could not take (other drive, no trash folder) */
  let noTrash = $state<DeleteTarget[]>([]);
  /** Items left after a permanent delete failed part-way */
  let left = $state<DeleteTarget[]>([]);
  let verified = $state(false);
  let password = $state('');
  let totp = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  const needs2FA = $derived($authStore.user?.totp_enabled ?? false);
  const pending = $derived(left.length > 0 ? left : noTrash.length > 0 ? noTrash : targets);
  const deleteForGood = $derived(permanent || noTrash.length > 0);
  const single = $derived(pending.length === 1 ? pending[0] : null);

  $effect(() => {
    if (!open) return;
    permanent = false;
    noTrash = [];
    left = [];
    password = '';
    totp = '';
    error = null;
    busy = false;
    api.system
      .deleteStatus()
      .then((s) => (verified = s.verified))
      .catch(() => (verified = false));
  });

  const title = $derived.by(() => {
    if (noTrash.length > 0) return m.filebrowser_no_trash_title();
    if (permanent) return m.filebrowser_delete_forever_title();
    return single
      ? m.filebrowser_trash_title_one({ name: single.name })
      : m.filebrowser_trash_title_many({ count: pending.length });
  });

  /** Delete for good; returns the deleted paths and the first error */
  async function deleteForever(paths: string[]): Promise<{ done: string[]; failure: string | null }> {
    if (!verified) {
      await api.system.verifyDeleteAccess(password, needs2FA ? totp : undefined);
      verified = true;
    }
    const done: string[] = [];
    let failure: string | null = null;
    for (const path of paths) {
      try {
        await api.system.deleteFile(path);
        done.push(path);
      } catch (err) {
        failure ??= err instanceof Error ? err.message : m.error_delete_failed();
      }
    }
    return { done, failure };
  }

  async function submit(e: Event) {
    e.preventDefault();
    error = null;
    if (deleteForGood && !verified && (!password || (needs2FA && !totp))) {
      error = m.filebrowser_delete_password_needed();
      return;
    }
    busy = true;
    try {
      if (deleteForGood) {
        const { done, failure } = await deleteForever(pending.map((t) => t.path));
        if (done.length > 0) onDeleted(done);
        if (failure) {
          left = pending.filter((t) => !done.includes(t.path));
          error = failure;
          return;
        }
        open = false;
        return;
      }
      const result = await api.files.trash(targets.map((t) => t.path));
      if (result.moved.length > 0) onTrashed(result.moved);
      const fallback = result.failed.filter((f) => f.reason === 'cross_device' || f.reason === 'no_anchor');
      const other = result.failed.filter((f) => f.reason !== 'cross_device' && f.reason !== 'no_anchor');
      if (fallback.length > 0) {
        // Ask once more, now for deleting these for good
        noTrash = targets.filter((t) => fallback.some((f) => f.path === t.path));
        if (other.length > 0) error = m.filebrowser_trash_some_failed({ count: other.length });
        return;
      }
      if (other.length > 0) {
        error =
          other[0].reason === 'protected'
            ? m.filebrowser_trash_protected()
            : m.filebrowser_trash_some_failed({ count: other.length });
        if (result.moved.length === 0) return;
      }
      open = false;
    } catch (err) {
      error = err instanceof Error ? err.message : m.error_delete_failed();
    } finally {
      busy = false;
    }
  }
</script>

<Dialog bind:open {title} size="sm">
  <form id="trash-form" onsubmit={submit} class="space-y-4">
    {#if noTrash.length > 0}
      <p class="text-sm text-gray-600 dark:text-gray-300">{m.filebrowser_no_trash_text({ count: noTrash.length })}</p>
    {:else}
      <p class="text-sm text-gray-600 dark:text-gray-300">
        {#if permanent}
          {single?.is_dir ? m.filebrowser_delete_forever_text_folder() : m.filebrowser_delete_forever_text()}
        {:else}
          {m.filebrowser_trash_text()}
        {/if}
      </p>
    {/if}

    {#if pending.length > 1}
      <ul class="max-h-36 overflow-y-auto text-sm rounded-lg border border-gray-200 dark:border-gray-700 divide-y divide-gray-100 dark:divide-gray-800">
        {#each pending as t (t.path)}
          <li class="flex items-center gap-2 px-3 py-1.5 min-w-0">
            <Icon name={t.is_dir ? 'folder' : 'file'} class="w-4 h-4 shrink-0 text-gray-400" />
            <span class="truncate">{t.name}</span>
          </li>
        {/each}
      </ul>
    {/if}

    {#if noTrash.length === 0}
      <label class="flex items-start gap-2.5 text-sm text-gray-700 dark:text-gray-300 cursor-pointer">
        <input type="checkbox" class="mt-0.5 rounded border-gray-300 dark:border-gray-600" bind:checked={permanent} />
        <span>{m.filebrowser_delete_forever_option()}</span>
      </label>
    {/if}

    {#if deleteForGood && !verified}
      <div class="space-y-3 rounded-lg bg-gray-50 dark:bg-gray-800/60 p-3">
        <p class="text-xs text-gray-500 dark:text-gray-400">{m.filebrowser_delete_verify_hint()}</p>
        <div>
          <label for="trash-password" class="label">{m.filebrowser_delete_password()}</label>
          <input id="trash-password" type="password" class="input" bind:value={password} autocomplete="current-password" />
        </div>
        {#if needs2FA}
          <div>
            <label for="trash-totp" class="label">{m.filebrowser_delete_totp()}</label>
            <input id="trash-totp" class="input font-mono tracking-widest" bind:value={totp} inputmode="numeric" autocomplete="one-time-code" maxlength="8" />
          </div>
        {/if}
      </div>
    {/if}

    {#if error}<p class="text-sm text-red-600 dark:text-red-400" role="alert">{error}</p>{/if}
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-secondary" onclick={() => (open = false)}>{m.common_cancel()}</button>
    <button type="submit" form="trash-form" class="btn {deleteForGood ? 'btn-danger' : 'btn-primary'}" disabled={busy}>
      {#if busy}<Icon name="refresh" class="w-4 h-4 animate-spin" />{/if}
      {deleteForGood ? m.filebrowser_delete_forever() : m.filebrowser_move_to_trash()}
    </button>
  {/snippet}
</Dialog>

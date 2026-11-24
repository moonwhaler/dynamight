<script lang="ts">
  import { fileTasks } from '$lib/files/tasks.svelte';
  import type { ConflictChoice, FileFacts } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import { formatModifiedFull } from '$lib/files/fileDates';
  import { getLocale } from '$lib/paraglide/runtime.js';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from '../ui/Icon.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // Opens by itself while a copy or move waits for an answer
  const task = $derived(fileTasks.conflict);
  const conflict = $derived(task?.conflict ?? null);
  let open = $state(false);
  let applyToAll = $state(false);
  let busy = $state(false);
  const locale = getLocale();

  $effect(() => {
    open = !!conflict;
    if (conflict) {
      applyToAll = false;
      busy = false;
    }
  });

  const name = $derived(conflict ? (conflict.path.split('/').pop() ?? conflict.path) : '');
  const folder = $derived(conflict ? conflict.path.split('/').slice(0, -1).join('/') : '');

  async function answer(choice: ConflictChoice) {
    if (!task) return;
    busy = true;
    try {
      await fileTasks.resolve(task.id, choice, applyToAll);
    } catch (e) {
      busy = false;
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    }
  }

  async function stop() {
    if (!task) return;
    busy = true;
    await fileTasks.cancel(task.id).catch(() => {});
  }
</script>

{#snippet facts(label: string, f: FileFacts)}
  <div class="flex-1 min-w-0 rounded-lg border border-gray-200 dark:border-gray-700 p-3">
    <div class="eyebrow mb-1.5">{label}</div>
    <div class="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200">
      <Icon name={f.is_dir ? 'folder' : 'file'} class="w-4 h-4 text-gray-400 shrink-0" />
      {f.is_dir ? m.filebrowser_type_folder() : formatBytes(f.size)}
    </div>
    <div class="mt-1 text-xs text-gray-500 dark:text-gray-400 tabular-nums">{formatModifiedFull(f.modified, locale)}</div>
  </div>
{/snippet}

<Dialog bind:open title={m.filebrowser_conflict_title()} size="md" dismissible={false} closeOnBackdrop={false}>
  {#if conflict}
    <div class="space-y-4">
      <p class="text-sm text-gray-600 dark:text-gray-300">
        {m.filebrowser_conflict_text({ name })}
        <span class="block font-mono text-xs text-gray-500 dark:text-gray-400 truncate mt-1" title={folder}>{folder}</span>
      </p>
      <div class="flex flex-col sm:flex-row gap-3">
        {@render facts(m.filebrowser_conflict_existing(), conflict.existing)}
        {@render facts(m.filebrowser_conflict_incoming(), conflict.incoming)}
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400">{m.filebrowser_conflict_replace_hint()}</p>
      <label class="flex items-center gap-2.5 text-sm text-gray-700 dark:text-gray-300 cursor-pointer">
        <input type="checkbox" class="rounded border-gray-300 dark:border-gray-600" bind:checked={applyToAll} />
        {m.filebrowser_conflict_all()}
      </label>
    </div>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn btn-ghost mr-auto" disabled={busy} onclick={stop}>{m.filebrowser_conflict_stop()}</button>
    <button type="button" class="btn btn-secondary" disabled={busy} onclick={() => answer('skip')}>{m.filebrowser_skip()}</button>
    <button type="button" class="btn btn-secondary" disabled={busy} onclick={() => answer('keep_both')}>{m.filebrowser_keep_both()}</button>
    <button type="button" class="btn btn-primary" disabled={busy} onclick={() => answer('replace')}>{m.filebrowser_replace()}</button>
  {/snippet}
</Dialog>

<script lang="ts">
  import { uploader, type UploadChoice } from '$lib/files/uploader.svelte';
  import { formatBytes } from '$lib/utils/format';
  import { formatModifiedFull } from '$lib/files/fileDates';
  import { getLocale } from '$lib/paraglide/runtime.js';
  import Dialog from '../ui/Dialog.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // Opens by itself while an upload waits for an answer
  const item = $derived(uploader.conflict);
  let open = $state(false);
  let applyToAll = $state(false);
  const locale = getLocale();

  $effect(() => {
    open = !!item;
    if (item) applyToAll = false;
  });

  function answer(choice: UploadChoice) {
    if (item) uploader.resolve(item.key, choice, applyToAll);
  }
</script>

<Dialog bind:open title={m.filebrowser_conflict_title()} size="md" dismissible={false} closeOnBackdrop={false}>
  {#if item}
    <div class="space-y-4">
      <p class="text-sm text-gray-600 dark:text-gray-300">{m.filebrowser_upload_conflict_text({ name: item.file.name })}</p>
      <div class="flex flex-col sm:flex-row gap-3 text-sm">
        <div class="flex-1 rounded-lg border border-gray-200 dark:border-gray-700 p-3">
          <div class="eyebrow mb-1.5">{m.filebrowser_conflict_existing()}</div>
          <div class="text-gray-800 dark:text-gray-200">{formatBytes(item.existing?.size ?? null)}</div>
          <div class="mt-1 text-xs text-gray-500 dark:text-gray-400 tabular-nums">{formatModifiedFull(item.existing?.modified ?? null, locale)}</div>
        </div>
        <div class="flex-1 rounded-lg border border-gray-200 dark:border-gray-700 p-3">
          <div class="eyebrow mb-1.5">{m.filebrowser_conflict_incoming()}</div>
          <div class="text-gray-800 dark:text-gray-200">{formatBytes(item.file.size)}</div>
          <div class="mt-1 text-xs text-gray-500 dark:text-gray-400 tabular-nums">{formatModifiedFull(Math.floor(item.file.lastModified / 1000), locale)}</div>
        </div>
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400">{m.filebrowser_conflict_replace_hint()}</p>
      <label class="flex items-center gap-2.5 text-sm text-gray-700 dark:text-gray-300 cursor-pointer">
        <input type="checkbox" class="rounded border-gray-300 dark:border-gray-600" bind:checked={applyToAll} />
        {m.filebrowser_conflict_all()}
      </label>
    </div>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn btn-ghost mr-auto" onclick={() => item && uploader.cancel(item.key)}>{m.common_cancel()}</button>
    <button type="button" class="btn btn-secondary" onclick={() => answer('skip')}>{m.filebrowser_skip()}</button>
    <button type="button" class="btn btn-secondary" onclick={() => answer('keep_both')}>{m.filebrowser_keep_both()}</button>
    <button type="button" class="btn btn-primary" onclick={() => answer('replace')}>{m.filebrowser_replace()}</button>
  {/snippet}
</Dialog>

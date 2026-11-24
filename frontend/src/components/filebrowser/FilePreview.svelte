<script lang="ts">
  import { api } from '$lib/api';
  import type { DirectoryEntry } from '$lib/types';
  import { previewKind } from '$lib/files/previewKind';
  import { formatModifiedFull } from '$lib/files/fileDates';
  import { formatBytes } from '$lib/utils/format';
  import { getFileIcon } from '$lib/utils/fileIcons';
  import { getLocale } from '$lib/paraglide/runtime.js';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    entry,
    hasPrev,
    hasNext,
    playing = false,
    onPrev,
    onNext,
    onClose,
    onDownload,
    onRename,
    onTrash,
    onCopyPath,
    onPlay,
  }: {
    entry: DirectoryEntry;
    hasPrev: boolean;
    hasNext: boolean;
    /** This (audio) file is the one playing */
    playing?: boolean;
    onPrev: () => void;
    onNext: () => void;
    onClose: () => void;
    onDownload: () => void;
    onRename: () => void;
    onTrash: () => void;
    onCopyPath: () => void;
    onPlay: () => void;
  } = $props();

  const locale = getLocale();
  const kind = $derived(previewKind(entry));
  const inline = $derived(api.files.rawUrl(entry.path, true));
  const icon = $derived(getFileIcon(entry.extension, entry.is_dir));

  // Text is fetched; a newer file wins over a slow older answer
  let text = $state<{ text: string; binary: boolean; truncated: boolean } | null>(null);
  let textError = $state<string | null>(null);
  let generation = 0;
  $effect(() => {
    const path = entry.path;
    text = null;
    textError = null;
    if (kind !== 'text') return;
    const mine = ++generation;
    api.files
      .text(path)
      .then((t) => {
        if (mine === generation) text = t;
      })
      .catch((e) => {
        if (mine === generation) textError = e instanceof Error ? e.message : m.error_generic();
      });
  });

  let mediaFailed = $state(false);
  $effect(() => {
    void entry.path;
    mediaFailed = false;
  });

  const lines = $derived(text && !text.binary ? text.text.split('\n') : []);
  /** Line numbers cost a node per line: only for shorter files */
  const numbered = $derived(lines.length > 0 && lines.length <= 5000);

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t.tagName === 'VIDEO') return;
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="flex flex-col h-full min-h-0" aria-label={m.filebrowser_preview_of({ name: entry.name })} role="region">
  <div class="flex items-start gap-2 px-4 py-3 border-b border-gray-200 dark:border-gray-700">
    <div class="min-w-0 flex-1">
      <h2 class="text-sm font-semibold text-gray-900 dark:text-white break-words">{entry.name}</h2>
      <p class="text-xs text-gray-500 dark:text-gray-400 tabular-nums mt-0.5">
        {formatBytes(entry.size)} · {formatModifiedFull(entry.modified, locale)}
      </p>
    </div>
    <div class="flex items-center shrink-0">
      <button type="button" class="btn btn-ghost btn-icon btn-sm" disabled={!hasPrev} onclick={onPrev} aria-label={m.filebrowser_previous_file()} title={m.filebrowser_previous_file()}>
        <Icon name="chevron-left" class="w-4 h-4" />
      </button>
      <button type="button" class="btn btn-ghost btn-icon btn-sm" disabled={!hasNext} onclick={onNext} aria-label={m.filebrowser_next_file()} title={m.filebrowser_next_file()}>
        <Icon name="chevron-right" class="w-4 h-4" />
      </button>
      <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={onClose} aria-label={m.common_close()} title={m.common_close()}>
        <Icon name="x" class="w-4 h-4" />
      </button>
    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-auto bg-gray-50 dark:bg-gray-900/40">
    {#if kind === 'image'}
      <a href={inline} target="_blank" rel="noopener" class="block p-3" title={m.filebrowser_open_new_tab()}>
        <img src={inline} alt={entry.name} class="mx-auto max-h-[70vh] object-contain rounded-md" onerror={() => (mediaFailed = true)} />
      </a>
      {#if mediaFailed}<p class="px-4 pb-4 text-sm text-gray-500">{m.filebrowser_preview_unsupported()}</p>{/if}
    {:else if kind === 'video'}
      {#if mediaFailed}
        <div class="p-6 text-center text-sm text-gray-600 dark:text-gray-400">
          <Icon name="video" class="w-10 h-10 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
          {m.filebrowser_video_unsupported()}
        </div>
      {:else}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video src={inline} controls preload="metadata" class="w-full max-h-[70vh] bg-black" onerror={() => (mediaFailed = true)}></video>
      {/if}
    {:else if kind === 'audio'}
      <div class="p-8 text-center">
        <Icon name="audio" class="w-14 h-14 mx-auto {icon.color} mb-4" strokeWidth={1.25} />
        <button type="button" class="btn btn-primary" onclick={onPlay} disabled={playing}>
          <Icon name="play" class="w-4 h-4" />{playing ? m.filebrowser_now_playing() : m.filebrowser_play()}
        </button>
      </div>
    {:else if kind === 'pdf'}
      <iframe src={inline} title={entry.name} class="w-full h-[70vh] bg-white"></iframe>
    {:else if kind === 'text'}
      {#if textError}
        <p class="p-4 text-sm text-red-600 dark:text-red-400">{textError}</p>
      {:else if !text}
        <div class="py-10 flex justify-center"><Icon name="refresh" class="w-5 h-5 text-primary-500 animate-spin" /></div>
      {:else if text.binary}
        <p class="p-4 text-sm text-gray-600 dark:text-gray-400">{m.filebrowser_preview_binary()}</p>
      {:else}
        <pre class="text-xs leading-5 font-mono text-gray-800 dark:text-gray-200 p-3 whitespace-pre-wrap break-words {numbered ? 'numbered' : ''}">{#if numbered}{#each lines as line, i (i)}<span>{line}</span>{/each}{:else}{text.text}{/if}</pre>
        {#if text.truncated}
          <p class="px-4 pb-4 text-xs text-gray-500 dark:text-gray-400">{m.filebrowser_preview_truncated()}</p>
        {/if}
      {/if}
    {:else}
      <div class="p-8 text-center text-sm text-gray-600 dark:text-gray-400">
        <Icon name={icon.icon} class="w-12 h-12 mx-auto {icon.color} mb-3" strokeWidth={1.25} />
        {m.filebrowser_preview_none()}
      </div>
    {/if}
  </div>

  <div class="px-4 py-2 border-t border-gray-200 dark:border-gray-700">
    <button
      type="button"
      class="w-full text-left text-xs font-mono text-gray-500 dark:text-gray-400 truncate hover:text-gray-800 dark:hover:text-gray-200"
      title={m.filebrowser_copy_path()}
      onclick={onCopyPath}>{entry.path}</button
    >
  </div>
  <div class="flex flex-wrap items-center gap-2 px-4 py-3 border-t border-gray-200 dark:border-gray-700">
    <button type="button" class="btn btn-secondary btn-sm" onclick={onDownload}><Icon name="download" class="w-4 h-4" />{m.filebrowser_download()}</button>
    {#if kind === 'image' || kind === 'pdf' || kind === 'video' || kind === 'text'}
      <a class="btn btn-ghost btn-sm" href={inline} target="_blank" rel="noopener"><Icon name="external" class="w-4 h-4" />{m.filebrowser_open_new_tab()}</a>
    {/if}
    <span class="flex-1"></span>
    <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={onRename} aria-label={m.filebrowser_rename()} title={m.filebrowser_rename()}>
      <Icon name="edit" class="w-4 h-4" />
    </button>
    <button type="button" class="btn btn-danger-ghost btn-icon btn-sm" onclick={onTrash} aria-label={m.filebrowser_move_to_trash()} title={m.filebrowser_move_to_trash()}>
      <Icon name="trash" class="w-4 h-4" />
    </button>
  </div>
</div>

<style>
  .numbered {
    counter-reset: line;
  }
  .numbered :global(span) {
    display: block;
    min-height: 1.25rem;
    padding-left: 3.5em;
    text-indent: -3.5em;
  }
  .numbered :global(span)::before {
    counter-increment: line;
    content: counter(line);
    display: inline-block;
    width: 2.75em;
    margin-right: 0.75em;
    text-align: right;
    color: rgb(156 163 175);
    user-select: none;
  }
</style>

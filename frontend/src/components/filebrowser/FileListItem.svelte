<script lang="ts">
  import type { DirectoryEntry } from '$lib/types';
  import { getFileIcon, formatFileSize, formatDate } from '$lib/utils/fileIcons';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    entry: DirectoryEntry;
    viewMode: 'list' | 'grid';
    onNavigate: (path: string) => void;
    onDownload: (path: string) => void;
    onDelete?: (path: string, name: string, isDir: boolean) => void;
    downloading?: string | null;
    deleting?: string | null;
    selectable?: boolean;
    selected?: boolean;
    onSelect?: (path: string) => void;
    visibleColumns?: string[];
    searchQuery?: string;
    clickableFiles?: boolean;
    basePath?: string;
  }

  let {
    entry,
    viewMode,
    onNavigate,
    onDownload,
    onDelete,
    downloading = null,
    deleting = null,
    selectable = false,
    selected = false,
    onSelect,
    visibleColumns = ['name', 'size', 'modified', 'actions'],
    searchQuery = '',
    clickableFiles = false,
    basePath = '',
  }: Props = $props();

  const relativePath = $derived.by(() => {
    if (!basePath) return '';
    const dir = entry.path.split('/').slice(0, -1).join('/');
    if (dir === basePath) return '';
    return dir.startsWith(basePath + '/') ? dir.slice(basePath.length + 1) : '';
  });

  function highlightSegments(text: string, query: string): { text: string; highlight: boolean }[] {
    const q = query.trim();
    if (!q) return [{ text, highlight: false }];
    const idx = text.toLowerCase().indexOf(q.toLowerCase());
    if (idx < 0) return [{ text, highlight: false }];
    const result: { text: string; highlight: boolean }[] = [];
    if (idx > 0) result.push({ text: text.slice(0, idx), highlight: false });
    result.push({ text: text.slice(idx, idx + q.length), highlight: true });
    if (idx + q.length < text.length) result.push({ text: text.slice(idx + q.length), highlight: false });
    return result;
  }

  const iconInfo = $derived(getFileIcon(entry.extension, entry.is_dir));
  const isDownloading = $derived(downloading === entry.path);
  const isDeleting = $derived(deleting === entry.path);

  const isClickable = $derived(entry.is_dir || clickableFiles || (selectable && onSelect));

  function handleClick() {
    if (entry.is_dir) {
      onNavigate(entry.path);
    } else if (clickableFiles) {
      onNavigate(entry.path);
    } else if (selectable && onSelect) {
      onSelect(entry.path);
    }
  }

  function handleDownload(e: Event) {
    e.stopPropagation();
    onDownload(entry.path);
  }

  function handleDelete(e: Event) {
    e.stopPropagation();
    onDelete?.(entry.path, entry.name, entry.is_dir);
  }
</script>

{#if viewMode === 'list'}
  <!-- List view row -->
  <tr
    class="hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors {isClickable ? 'cursor-pointer' : ''} {selected ? 'bg-primary-50 dark:bg-primary-900/20' : ''}"
    onclick={handleClick}
    role={isClickable ? 'button' : undefined}
    tabindex={isClickable ? 0 : undefined}
    onkeydown={(e) => e.key === 'Enter' && handleClick()}
  >
    <td class="px-4 py-2.5">
      <div class="flex items-center gap-3">
        <!-- Icon -->
        <div class="flex-shrink-0">
          {#if iconInfo.icon === 'folder'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
            </svg>
          {:else if iconInfo.icon === 'image'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
          {:else if iconInfo.icon === 'document'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
          {:else if iconInfo.icon === 'archive'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
            </svg>
          {:else if iconInfo.icon === 'code'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
            </svg>
          {:else if iconInfo.icon === 'video'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
          {:else if iconInfo.icon === 'audio'}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
            </svg>
          {:else}
            <svg class="w-5 h-5 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
            </svg>
          {/if}
        </div>

        <!-- Name -->
        <div class="truncate">
          <span class="text-gray-900 dark:text-white {entry.is_dir ? 'font-medium' : ''}">
            {#each highlightSegments(entry.name, searchQuery) as seg (seg.text + seg.highlight)}
              {#if seg.highlight}
                <mark class="bg-yellow-200 dark:bg-yellow-800/50 text-inherit rounded-sm not-italic">{seg.text}</mark>
              {:else}{seg.text}{/if}
            {/each}
          </span>
          {#if relativePath}
            <span class="text-xs text-gray-400 dark:text-gray-500 truncate block">
              {relativePath}
            </span>
          {/if}
        </div>
      </div>
    </td>

    {#if visibleColumns.includes('size')}
      <td class="px-4 py-2.5 text-right text-gray-500 dark:text-gray-400 text-sm">
        {entry.is_dir ? '-' : formatFileSize(entry.size)}
      </td>
    {/if}

    {#if visibleColumns.includes('modified')}
      <td class="px-4 py-2.5 text-right text-gray-500 dark:text-gray-400 text-sm">
        {formatDate(entry.modified)}
      </td>
    {/if}

    <td class="px-4 py-2.5 text-right">
      <div class="flex items-center justify-end gap-1">
        {#if !entry.is_dir}
          <button
            type="button"
            onclick={handleDownload}
            disabled={isDownloading || isDeleting}
            class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-600 text-gray-500 dark:text-gray-400 hover:text-primary-600 dark:hover:text-primary-400 disabled:opacity-50"
            title={m.filebrowser_download()}
          >
            {#if isDownloading}
              <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
              </svg>
            {:else}
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
            {/if}
          </button>
        {/if}
        {#if onDelete}
          <button
            type="button"
            onclick={handleDelete}
            disabled={isDownloading || isDeleting}
            class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-600 text-gray-500 dark:text-gray-400 hover:text-red-600 dark:hover:text-red-400 disabled:opacity-50"
            title={m.filebrowser_delete()}
          >
            {#if isDeleting}
              <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
              </svg>
            {:else}
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            {/if}
          </button>
        {/if}
      </div>
    </td>
  </tr>
{:else}
  <!-- Grid view card -->
  {#if entry.is_dir}
    <div class="card p-4 flex flex-col items-center gap-2 text-center relative {selected ? 'ring-2 ring-primary-500' : ''}">
      <!-- Click area for navigation -->
      <button
        type="button"
        onclick={handleClick}
        class="w-full flex flex-col items-center gap-2 hover:opacity-80 transition-opacity cursor-pointer"
      >
        <!-- Icon -->
        <div class="w-12 h-12 flex items-center justify-center">
          <svg class="w-10 h-10 {iconInfo.color}" fill="currentColor" viewBox="0 0 20 20">
            <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
          </svg>
        </div>

        <!-- Name -->
        <span class="text-sm text-gray-900 dark:text-white truncate w-full font-medium">
          {#each highlightSegments(entry.name, searchQuery) as seg (seg.text + seg.highlight)}
            {#if seg.highlight}
              <mark class="bg-yellow-200 dark:bg-yellow-800/50 text-inherit rounded-sm not-italic">{seg.text}</mark>
            {:else}{seg.text}{/if}
          {/each}
        </span>
        {#if relativePath}
          <span class="text-xs text-gray-400 dark:text-gray-500 truncate w-full">{relativePath}</span>
        {/if}
      </button>

      <!-- Delete button for folders -->
      {#if onDelete}
        <button
          type="button"
          onclick={handleDelete}
          disabled={isDeleting}
          class="mt-1 px-3 py-1 text-xs rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-red-100 dark:hover:bg-red-900/30 hover:text-red-600 dark:hover:text-red-400 disabled:opacity-50"
        >
          {#if isDeleting}
            {m.filebrowser_deleting()}
          {:else}
            {m.filebrowser_delete()}
          {/if}
        </button>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="card p-4 flex flex-col items-center gap-2 text-center {selected ? 'ring-2 ring-primary-500' : ''} {clickableFiles ? 'cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700/50' : ''}"
      onclick={clickableFiles ? handleClick : undefined}
      role={clickableFiles ? 'button' : undefined}
      tabindex={clickableFiles ? 0 : undefined}
      onkeydown={clickableFiles ? (e) => e.key === 'Enter' && handleClick() : undefined}
    >
      <!-- Icon -->
      <div class="w-12 h-12 flex items-center justify-center">
        <svg class="w-10 h-10 {iconInfo.color}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
        </svg>
      </div>

      <!-- Name -->
      <span class="text-sm text-gray-900 dark:text-white truncate w-full">
        {#each highlightSegments(entry.name, searchQuery) as seg (seg.text + seg.highlight)}
          {#if seg.highlight}
            <mark class="bg-yellow-200 dark:bg-yellow-800/50 text-inherit rounded-sm not-italic">{seg.text}</mark>
          {:else}{seg.text}{/if}
        {/each}
      </span>
      {#if relativePath}
        <span class="text-xs text-gray-400 dark:text-gray-500 truncate w-full">{relativePath}</span>
      {/if}

      <!-- Size -->
      <span class="text-xs text-gray-500 dark:text-gray-400">
        {formatFileSize(entry.size)}
      </span>

      <!-- Action buttons -->
      <div class="mt-1 flex gap-2">
        <button
          type="button"
          onclick={handleDownload}
          disabled={isDownloading || isDeleting}
          class="px-3 py-1 text-xs rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-primary-100 dark:hover:bg-primary-900/30 hover:text-primary-600 dark:hover:text-primary-400 disabled:opacity-50"
        >
          {#if isDownloading}
            {m.filebrowser_downloading()}
          {:else}
            {m.filebrowser_download()}
          {/if}
        </button>
        {#if onDelete}
          <button
            type="button"
            onclick={handleDelete}
            disabled={isDownloading || isDeleting}
            class="px-3 py-1 text-xs rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-red-100 dark:hover:bg-red-900/30 hover:text-red-600 dark:hover:text-red-400 disabled:opacity-50"
          >
            {#if isDeleting}
              {m.filebrowser_deleting()}
            {:else}
              {m.filebrowser_delete()}
            {/if}
          </button>
        {/if}
      </div>
    </div>
  {/if}
{/if}

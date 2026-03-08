<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { DirectoryEntry } from '$lib/types';
  import type { SortField, SortOrder, ViewMode } from '$lib/stores/fileBrowser';
  import { fileBrowserTablePreferencesStore, FB_FIXED } from '$lib/stores/fileBrowserTablePreferences';
  import type { FileBrowserColumnKey } from '$lib/stores/fileBrowserTablePreferences';
  import FileListItem from './FileListItem.svelte';
  import SortIcon from '../ui/SortIcon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    entries: DirectoryEntry[];
    viewMode: ViewMode;
    sortBy: SortField;
    sortOrder: SortOrder;
    loading: boolean;
    downloading?: string | null;
    deleting?: string | null;
    error?: string | null;
    onNavigate: (path: string) => void;
    onDownload: (path: string) => void;
    onDelete?: (path: string, name: string, isDir: boolean) => void;
    onSortChange?: (field: SortField) => void;
    onSortOrderToggle?: () => void;
    selectable?: boolean;
    selectedPath?: string | null;
    onSelect?: (path: string) => void;
    searchQuery?: string;
  }

  let {
    entries,
    viewMode,
    sortBy,
    sortOrder,
    loading,
    downloading = null,
    deleting = null,
    error = null,
    onNavigate,
    onDownload,
    onDelete,
    onSortChange,
    onSortOrderToggle,
    selectable = false,
    selectedPath = null,
    onSelect,
    searchQuery = '',
  }: Props = $props();

  let tableContainerEl = $state<HTMLElement | null>(null);
  let containerWidth = $state(0);
  let dragCol = $state<FileBrowserColumnKey | null>(null);
  let dragOverCol = $state<FileBrowserColumnKey | null>(null);
  let resizing: { col: FileBrowserColumnKey; startX: number; startWidth: number } | null = null;

  const storedTotalWidth = $derived(
    $fileBrowserTablePreferencesStore.visibleColumns.reduce(
      (sum, col) => sum + $fileBrowserTablePreferencesStore.columnWidths[col],
      0
    )
  );

  const effectiveWidths = $derived.by<Record<FileBrowserColumnKey, number>>(() => {
    const cols = $fileBrowserTablePreferencesStore.visibleColumns;
    const stored = $fileBrowserTablePreferencesStore.columnWidths;
    const extra = containerWidth - storedTotalWidth;
    if (extra <= 0 || storedTotalWidth === 0) return { ...stored };
    const result = { ...stored };
    for (const col of cols) {
      result[col] = Math.round(stored[col] + extra * (stored[col] / storedTotalWidth));
    }
    return result;
  });

  $effect(() => {
    if (!tableContainerEl) return;
    const ro = new ResizeObserver((entries) => { containerWidth = entries[0].contentRect.width; });
    ro.observe(tableContainerEl);
    return () => ro.disconnect();
  });

  function handleSort(field: SortField) {
    if (sortBy === field && onSortOrderToggle) {
      onSortOrderToggle();
    } else if (onSortChange) {
      onSortChange(field);
    }
  }

  function startResize(col: FileBrowserColumnKey, e: MouseEvent) {
    e.preventDefault();
    resizing = { col, startX: e.clientX, startWidth: effectiveWidths[col] };
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', onResizeMove);
    window.addEventListener('mouseup', onResizeEnd);
  }

  function onResizeMove(e: MouseEvent) {
    if (!resizing) return;
    fileBrowserTablePreferencesStore.setColumnWidth(resizing.col, resizing.startWidth + (e.clientX - resizing.startX));
  }

  function onResizeEnd() {
    resizing = null;
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
  }

  function onDragStart(col: FileBrowserColumnKey, e: DragEvent) {
    dragCol = col;
    e.dataTransfer!.effectAllowed = 'move';
  }

  function onDragOver(col: FileBrowserColumnKey, e: DragEvent) {
    if (!dragCol || dragCol === col || FB_FIXED.includes(col)) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
    dragOverCol = col;
  }

  function onDrop(col: FileBrowserColumnKey, e: DragEvent) {
    e.preventDefault();
    if (!dragCol || dragCol === col || FB_FIXED.includes(col)) return;
    const cols = [...$fileBrowserTablePreferencesStore.visibleColumns];
    const fromIdx = cols.indexOf(dragCol);
    const toIdx = cols.indexOf(col);
    if (fromIdx < 0 || toIdx < 0) return;
    cols.splice(fromIdx, 1);
    cols.splice(fromIdx < toIdx ? toIdx - 1 : toIdx, 0, dragCol);
    fileBrowserTablePreferencesStore.setColumnOrder(cols);
    dragCol = null;
    dragOverCol = null;
  }

  function onDragEnd() {
    dragCol = null;
    dragOverCol = null;
  }

  onDestroy(() => {
    window.removeEventListener('mousemove', onResizeMove);
    window.removeEventListener('mouseup', onResizeEnd);
    document.body.style.userSelect = '';
  });
</script>

{#if loading}
  <div class="flex items-center justify-center py-12">
    <svg class="w-8 h-8 animate-spin text-primary-500" fill="none" viewBox="0 0 24 24">
      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
    </svg>
  </div>
{:else if error}
  <div class="p-6 text-center">
    <div class="w-12 h-12 mx-auto mb-4 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
      <svg class="w-6 h-6 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
    </div>
    <p class="text-gray-600 dark:text-gray-400">{error}</p>
  </div>
{:else if entries.length === 0}
  <div class="p-6 text-center">
    <div class="w-12 h-12 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
      {#if searchQuery.trim()}
        <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
      {:else}
        <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
        </svg>
      {/if}
    </div>
    {#if searchQuery.trim()}
      <p class="text-gray-500 dark:text-gray-400">
        {m.filebrowser_search_no_results({ query: searchQuery.trim() })}
      </p>
    {:else}
      <p class="text-gray-500 dark:text-gray-400">{m.filebrowser_empty()}</p>
    {/if}
  </div>
{:else if viewMode === 'list'}
  <!-- List view with column management -->
  <div class="overflow-x-auto" bind:this={tableContainerEl}>
    <table
      class="w-full table-fixed"
      style="width: {Math.max(storedTotalWidth, containerWidth)}px"
    >
      <colgroup>
        {#each $fileBrowserTablePreferencesStore.visibleColumns as col (col)}
          <col style="width: {effectiveWidths[col]}px" />
        {/each}
      </colgroup>
      <thead class="bg-gray-50 dark:bg-gray-800/50 text-left text-sm">
        <tr>
          {#each $fileBrowserTablePreferencesStore.visibleColumns as col (col)}
            <th
              class="px-4 py-3 font-medium text-gray-600 dark:text-gray-300 relative select-none
                {!FB_FIXED.includes(col) ? 'cursor-grab' : ''}
                {dragOverCol === col ? 'bg-primary-50 dark:bg-primary-900/20' : ''}"
              style="width: {effectiveWidths[col]}px"
              draggable={!FB_FIXED.includes(col)}
              ondragstart={!FB_FIXED.includes(col) ? (e) => onDragStart(col, e) : undefined}
              ondragover={(e) => onDragOver(col, e)}
              ondragleave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node) && dragOverCol === col) dragOverCol = null; }}
              ondrop={(e) => onDrop(col, e)}
              ondragend={onDragEnd}
            >
              {#if col === 'name'}
                <button
                  type="button"
                  onclick={() => handleSort('name')}
                  class="flex items-center gap-1 hover:text-gray-900 dark:hover:text-white"
                >
                  {m.filebrowser_column_name()}
                  <SortIcon active={sortBy === 'name'} order={sortOrder} />
                </button>
              {:else if col === 'size'}
                <div class="text-right">
                  <button
                    type="button"
                    onclick={() => handleSort('size')}
                    class="flex items-center gap-1 ml-auto hover:text-gray-900 dark:hover:text-white"
                  >
                    {m.filebrowser_column_size()}
                    <SortIcon active={sortBy === 'size'} order={sortOrder} />
                  </button>
                </div>
              {:else if col === 'modified'}
                <div class="text-right">
                  <button
                    type="button"
                    onclick={() => handleSort('modified')}
                    class="flex items-center gap-1 ml-auto hover:text-gray-900 dark:hover:text-white"
                  >
                    {m.filebrowser_column_modified()}
                    <SortIcon active={sortBy === 'modified'} order={sortOrder} />
                  </button>
                </div>
              {:else if col === 'actions'}
                <span class="sr-only">{m.filebrowser_column_actions()}</span>
              {/if}
              {#if dragOverCol === col}
                <div class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary-500 pointer-events-none"></div>
              {/if}
              {#if col !== 'actions'}
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <div
                  class="absolute right-0 top-0 bottom-0 w-3 translate-x-1/2 z-10 cursor-col-resize group"
                  onmousedown={(e) => startResize(col, e)}
                  ondragstart={(e) => e.stopPropagation()}
                  role="separator"
                  aria-orientation="vertical"
                >
                  <div class="absolute left-1/2 -translate-x-1/2 top-0 bottom-0 w-px bg-gray-200 dark:bg-gray-700 group-hover:bg-primary-500 dark:group-hover:bg-primary-400 transition-colors duration-150"></div>
                </div>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody class="divide-y divide-gray-100 dark:divide-gray-700">
        {#each entries as entry (entry.path)}
          <FileListItem
            {entry}
            {viewMode}
            {onNavigate}
            {onDownload}
            {onDelete}
            {downloading}
            {deleting}
            {selectable}
            selected={selectedPath === entry.path}
            {onSelect}
            {searchQuery}
            visibleColumns={$fileBrowserTablePreferencesStore.visibleColumns}
          />
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <!-- Grid view -->
  <div class="p-4 grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
    {#each entries as entry (entry.path)}
      <FileListItem
        {entry}
        {viewMode}
        {onNavigate}
        {onDownload}
        {onDelete}
        {downloading}
        {deleting}
        {selectable}
        selected={selectedPath === entry.path}
        {onSelect}
        {searchQuery}
      />
    {/each}
  </div>
{/if}

<script lang="ts">
  import type { DirectoryEntry } from '$lib/types';
  import type { SortField, SortOrder, ViewMode } from '$lib/stores/fileBrowser';
  import FileListItem from './FileListItem.svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    entries: DirectoryEntry[];
    viewMode: ViewMode;
    sortBy: SortField;
    sortOrder: SortOrder;
    loading: boolean;
    downloading?: string | null;
    error?: string | null;
    onNavigate: (path: string) => void;
    onDownload: (path: string) => void;
    onSortChange?: (field: SortField) => void;
    onSortOrderToggle?: () => void;
    selectable?: boolean;
    selectedPath?: string | null;
    onSelect?: (path: string) => void;
  }

  let {
    entries,
    viewMode,
    sortBy,
    sortOrder,
    loading,
    downloading = null,
    error = null,
    onNavigate,
    onDownload,
    onSortChange,
    onSortOrderToggle,
    selectable = false,
    selectedPath = null,
    onSelect,
  }: Props = $props();

  function handleSort(field: SortField) {
    if (sortBy === field && onSortOrderToggle) {
      onSortOrderToggle();
    } else if (onSortChange) {
      onSortChange(field);
    }
  }

  function getSortIcon(field: SortField) {
    if (sortBy !== field) return null;
    return sortOrder === 'asc' ? '↑' : '↓';
  }
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
      <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
      </svg>
    </div>
    <p class="text-gray-500 dark:text-gray-400">{m.filebrowser_empty()}</p>
  </div>
{:else if viewMode === 'list'}
  <!-- List view -->
  <div class="overflow-x-auto">
    <table class="w-full">
      <thead class="bg-gray-50 dark:bg-gray-800/50 text-left text-sm">
        <tr>
          <th class="px-4 py-3 font-medium text-gray-600 dark:text-gray-300">
            <button
              type="button"
              onclick={() => handleSort('name')}
              class="flex items-center gap-1 hover:text-gray-900 dark:hover:text-white"
            >
              {m.filebrowser_column_name()}
              {#if getSortIcon('name')}
                <span class="text-primary-500">{getSortIcon('name')}</span>
              {/if}
            </button>
          </th>
          <th class="px-4 py-3 font-medium text-gray-600 dark:text-gray-300 text-right hidden sm:table-cell">
            <button
              type="button"
              onclick={() => handleSort('size')}
              class="flex items-center gap-1 ml-auto hover:text-gray-900 dark:hover:text-white"
            >
              {m.filebrowser_column_size()}
              {#if getSortIcon('size')}
                <span class="text-primary-500">{getSortIcon('size')}</span>
              {/if}
            </button>
          </th>
          <th class="px-4 py-3 font-medium text-gray-600 dark:text-gray-300 text-right hidden md:table-cell">
            <button
              type="button"
              onclick={() => handleSort('modified')}
              class="flex items-center gap-1 ml-auto hover:text-gray-900 dark:hover:text-white"
            >
              {m.filebrowser_column_modified()}
              {#if getSortIcon('modified')}
                <span class="text-primary-500">{getSortIcon('modified')}</span>
              {/if}
            </button>
          </th>
          <th class="px-4 py-3 font-medium text-gray-600 dark:text-gray-300 text-right w-16">
            <span class="sr-only">{m.filebrowser_column_actions()}</span>
          </th>
        </tr>
      </thead>
      <tbody class="divide-y divide-gray-100 dark:divide-gray-700">
        {#each entries as entry (entry.path)}
          <FileListItem
            {entry}
            {viewMode}
            {onNavigate}
            {onDownload}
            {downloading}
            {selectable}
            selected={selectedPath === entry.path}
            {onSelect}
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
        {downloading}
        {selectable}
        selected={selectedPath === entry.path}
        {onSelect}
      />
    {/each}
  </div>
{/if}

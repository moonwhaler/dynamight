<script lang="ts">
  import { api } from '../../lib/api';
  import type { DirectoryEntry } from '../../lib/types';

  let { path = $bindable(''), placeholder = 'Enter path...' }: { path: string; placeholder?: string } = $props();

  let showBrowser = $state(false);
  let currentPath = $state('/');
  let entries = $state<DirectoryEntry[]>([]);
  let loading = $state(false);
  let browseError = $state<string | null>(null);

  // New folder creation state
  let creatingFolder = $state(false);
  let newFolderName = $state('');
  let createError = $state<string | null>(null);
  let newFolderInput = $state<HTMLInputElement | null>(null);

  async function browse(targetPath: string) {
    loading = true;
    browseError = null;
    creatingFolder = false;
    newFolderName = '';
    try {
      const result = await api.system.browse(targetPath);
      currentPath = result.path;
      entries = result.entries;
    } catch {
      // Path doesn't exist - try parent
      const parent = targetPath.split('/').slice(0, -1).join('/') || '/';
      if (parent !== targetPath) {
        try {
          const result = await api.system.browse(parent);
          currentPath = result.path;
          entries = result.entries;
          browseError = `"${targetPath}" doesn't exist. Showing parent directory.`;
        } catch {
          entries = [];
          browseError = 'Cannot access this path';
        }
      } else {
        entries = [];
        browseError = 'Cannot access this path';
      }
    } finally {
      loading = false;
    }
  }

  function openBrowser() {
    showBrowser = true;
    browseError = null;
    createError = null;
    // Start from the current path value, or root if empty
    const startPath = path.trim() || '/';
    browse(startPath);
  }

  function closeBrowser() {
    showBrowser = false;
    browseError = null;
    creatingFolder = false;
    newFolderName = '';
    createError = null;
  }

  function selectPath(selectedPath: string) {
    path = selectedPath;
    closeBrowser();
  }

  function goUp() {
    const parent = currentPath.split('/').slice(0, -1).join('/') || '/';
    browse(parent);
  }

  function startCreatingFolder() {
    creatingFolder = true;
    createError = null;
    newFolderName = '';
    // Focus the input after it renders
    setTimeout(() => newFolderInput?.focus(), 0);
  }

  function cancelCreatingFolder() {
    creatingFolder = false;
    newFolderName = '';
    createError = null;
  }

  async function createFolder() {
    const name = newFolderName.trim();
    if (!name) return;

    // Validate folder name
    if (name.includes('/') || name.includes('\\') || name === '.' || name === '..') {
      createError = 'Invalid folder name';
      return;
    }

    const newPath = currentPath === '/' ? `/${name}` : `${currentPath}/${name}`;

    try {
      await api.system.mkdir(newPath);
      creatingFolder = false;
      newFolderName = '';
      createError = null;
      // Refresh and navigate into the new folder
      await browse(newPath);
    } catch (e) {
      createError = e instanceof Error ? e.message : 'Failed to create folder';
    }
  }

  function handleNewFolderKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      createFolder();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelCreatingFolder();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      closeBrowser();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !creatingFolder) {
      closeBrowser();
    }
  }
</script>

<svelte:window onkeydown={showBrowser ? handleKeydown : undefined} />

<div class="flex gap-2">
  <input
    type="text"
    bind:value={path}
    {placeholder}
    class="input flex-1"
  />
  <button type="button" onclick={openBrowser} class="btn btn-secondary">Browse</button>
</div>

<!-- File Browser Modal -->
{#if showBrowser}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={(e) => e.key === 'Escape' && closeBrowser()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col">
      <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Browse Filesystem</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400 font-mono">{currentPath}</p>
        </div>
        <button onclick={closeBrowser} class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300" aria-label="Close browser">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {#if browseError}
        <div class="mx-4 mt-3 px-3 py-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-400 text-sm rounded-lg">
          {browseError}
        </div>
      {/if}

      <div class="p-3 border-b border-gray-200 dark:border-gray-700 flex gap-2">
        <button onclick={goUp} disabled={currentPath === '/'} class="btn btn-secondary text-sm inline-flex items-center">
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 11l5-5m0 0l5 5m-5-5v12" />
          </svg>
          Up
        </button>
        <button onclick={startCreatingFolder} disabled={creatingFolder} class="btn btn-secondary text-sm inline-flex items-center">
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          New Folder
        </button>
        <button onclick={() => selectPath(currentPath)} class="btn btn-primary text-sm ml-auto">
          Select This Directory
        </button>
      </div>

      <div class="flex-1 overflow-auto p-2 min-h-[200px]">
        {#if creatingFolder}
          <div class="mb-2 p-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
            <div class="flex items-center gap-2">
              <svg class="w-5 h-5 text-blue-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
              </svg>
              <input
                bind:this={newFolderInput}
                bind:value={newFolderName}
                onkeydown={handleNewFolderKeydown}
                placeholder="Folder name"
                class="flex-1 px-2 py-1 text-sm border border-blue-300 dark:border-blue-600 rounded focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
              />
              <button onclick={createFolder} class="btn btn-primary text-xs py-1 px-2">Create</button>
              <button onclick={cancelCreatingFolder} class="btn btn-secondary text-xs py-1 px-2">Cancel</button>
            </div>
            {#if createError}
              <p class="mt-1 text-xs text-red-600 dark:text-red-400">{createError}</p>
            {/if}
          </div>
        {/if}

        {#if loading}
          <div class="flex justify-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if entries.length === 0 && !creatingFolder}
          <div class="text-center py-8 text-gray-500 dark:text-gray-400">
            <svg class="w-12 h-12 mx-auto mb-2 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <p>Directory is empty</p>
          </div>
        {:else if !loading}
          <div class="space-y-0.5">
            {#each entries as entry}
              {#if entry.is_dir}
                <button
                  onclick={() => browse(entry.path)}
                  class="w-full flex items-center gap-2 px-3 py-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg text-left transition-colors"
                >
                  <svg class="w-5 h-5 text-yellow-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                  </svg>
                  <span class="text-gray-900 dark:text-gray-100 truncate">{entry.name}</span>
                </button>
              {:else}
                <div class="flex items-center gap-2 px-3 py-2 text-gray-400">
                  <svg class="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                  <span class="truncate">{entry.name}</span>
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>

      <div class="p-3 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 rounded-b-xl">
        <p class="text-xs text-gray-500 dark:text-gray-400">
          Tip: You can type any path in the input field, even if it doesn't exist yet.
        </p>
      </div>
    </div>
  </div>
{/if}

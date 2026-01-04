<script lang="ts">
  import { api } from '../../lib/api';
  import type { DirectoryEntry } from '../../lib/types';

  let { path = $bindable(''), placeholder = 'Enter path...' }: { path: string; placeholder?: string } = $props();

  let showBrowser = $state(false);
  let currentPath = $state('/');
  let entries = $state<DirectoryEntry[]>([]);
  let loading = $state(false);
  let browseError = $state<string | null>(null);
  let allowedPaths = $state<string[]>([]);
  let showingRoots = $state(false);

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
    showingRoots = false;
    try {
      const result = await api.system.browse(targetPath);
      currentPath = result.path;
      entries = result.entries;
    } catch {
      // Path doesn't exist or not allowed - try to show allowed roots
      if (allowedPaths.length > 1) {
        showingRoots = true;
        currentPath = '/';
        entries = allowedPaths.map(p => ({
          name: p,
          path: p,
          is_dir: true,
          size: null,
        }));
        browseError = `"${targetPath}" is not accessible. Select an allowed path below.`;
      } else if (allowedPaths.length === 1) {
        // Try the single allowed path
        try {
          const result = await api.system.browse(allowedPaths[0]);
          currentPath = result.path;
          entries = result.entries;
          browseError = `"${targetPath}" is not accessible. Showing ${allowedPaths[0]}.`;
        } catch {
          entries = [];
          browseError = 'Cannot access any allowed paths';
        }
      } else {
        entries = [];
        browseError = 'Cannot access this path';
      }
    } finally {
      loading = false;
    }
  }

  async function openBrowser() {
    showBrowser = true;
    browseError = null;
    createError = null;
    loading = true;

    try {
      // Fetch allowed paths first
      const result = await api.system.allowedPaths();
      allowedPaths = result.paths;

      // Start from the current path value if it's set and valid
      const startPath = path.trim();
      if (startPath && allowedPaths.some(allowed => startPath.startsWith(allowed))) {
        await browse(startPath);
      } else if (allowedPaths.length === 1) {
        // Only one allowed path, go directly to it
        await browse(allowedPaths[0]);
      } else if (allowedPaths.length > 1) {
        // Multiple allowed paths, show them as selectable roots
        showingRoots = true;
        currentPath = '/';
        entries = allowedPaths.map(p => ({
          name: p,
          path: p,
          is_dir: true,
          size: null,
        }));
        loading = false;
      } else {
        // No allowed paths configured
        entries = [];
        browseError = 'No browseable paths configured';
        loading = false;
      }
    } catch {
      entries = [];
      browseError = 'Failed to load allowed paths';
      loading = false;
    }
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
    // Check if we're at an allowed root path
    const isAtRoot = allowedPaths.includes(currentPath);

    if (isAtRoot && allowedPaths.length > 1) {
      // Go back to showing root options
      showingRoots = true;
      currentPath = '/';
      entries = allowedPaths.map(p => ({
        name: p,
        path: p,
        is_dir: true,
        size: null,
      }));
      browseError = null;
    } else if (isAtRoot) {
      // Only one allowed path, can't go higher
      return;
    } else {
      // Navigate to parent directory
      const parent = currentPath.split('/').slice(0, -1).join('/') || '/';
      // Check if parent is still within allowed paths
      const isParentAllowed = allowedPaths.some(allowed => parent.startsWith(allowed) || parent === allowed);
      if (isParentAllowed) {
        browse(parent);
      } else if (allowedPaths.length > 1) {
        // Go back to showing root options
        showingRoots = true;
        currentPath = '/';
        entries = allowedPaths.map(p => ({
          name: p,
          path: p,
          is_dir: true,
          size: null,
        }));
        browseError = null;
      }
    }
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
    class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={(e) => e.key === 'Escape' && closeBrowser()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl max-w-2xl w-full h-[70vh] flex flex-col overflow-hidden">
      <!-- Header -->
      <div class="px-5 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div class="min-w-0 flex-1">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Select Directory</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400 font-mono truncate mt-0.5">{currentPath}</p>
        </div>
        <button
          onclick={closeBrowser}
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-xl transition-colors ml-3"
          aria-label="Close browser"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Error Message -->
      {#if browseError}
        <div class="mx-4 mt-3 px-4 py-2.5 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-400 text-sm rounded-xl flex-shrink-0">
          {browseError}
        </div>
      {/if}

      <!-- Navigation & Actions -->
      <div class="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex gap-2 items-center flex-shrink-0">
        <button
          type="button"
          onclick={goUp}
          disabled={showingRoots || (allowedPaths.includes(currentPath) && allowedPaths.length === 1)}
          class="btn btn-secondary text-sm inline-flex items-center"
        >
          <svg class="w-4 h-4 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 11l5-5m0 0l5 5m-5-5v12" />
          </svg>
          Up
        </button>
        <button
          type="button"
          onclick={startCreatingFolder}
          disabled={creatingFolder}
          class="btn btn-secondary text-sm inline-flex items-center"
        >
          <svg class="w-4 h-4 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          New Folder
        </button>
        <div class="flex-1"></div>
        <button
          type="button"
          onclick={() => selectPath(currentPath)}
          class="btn btn-primary text-sm inline-flex items-center"
        >
          <svg class="w-4 h-4 mr-1.5" fill="currentColor" viewBox="0 0 20 20">
            <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
          </svg>
          Select This Directory
        </button>
      </div>

      <!-- Directory Listing -->
      <div class="flex-1 min-h-0 overflow-y-auto p-3">
        {#if creatingFolder}
          <div class="mb-3 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl">
            <div class="flex items-center gap-3">
              <svg class="w-5 h-5 text-blue-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
              </svg>
              <input
                bind:this={newFolderInput}
                bind:value={newFolderName}
                onkeydown={handleNewFolderKeydown}
                placeholder="Folder name"
                class="flex-1 px-3 py-1.5 text-sm border border-blue-300 dark:border-blue-600 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
              />
              <button type="button" onclick={createFolder} class="btn btn-primary text-sm py-1.5 px-3">Create</button>
              <button type="button" onclick={cancelCreatingFolder} class="btn btn-secondary text-sm py-1.5 px-3">Cancel</button>
            </div>
            {#if createError}
              <p class="mt-2 text-xs text-red-600 dark:text-red-400">{createError}</p>
            {/if}
          </div>
        {/if}

        {#if loading}
          <div class="flex items-center justify-center h-full">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if entries.length === 0 && !creatingFolder}
          <div class="flex flex-col items-center justify-center h-full text-gray-500 dark:text-gray-400">
            <svg class="w-12 h-12 mb-3 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
                  class="w-full flex items-center gap-2.5 px-3 py-2.5 hover:bg-gray-100 dark:hover:bg-gray-700/50 rounded-xl text-left transition-colors"
                >
                  <svg class="w-5 h-5 text-yellow-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                  </svg>
                  <span class="text-gray-900 dark:text-gray-100 truncate">{entry.name}</span>
                </button>
              {:else}
                <div class="flex items-center gap-2.5 px-3 py-2.5 text-gray-400 dark:text-gray-500">
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

      <!-- Footer -->
      <div class="px-5 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex items-center justify-between gap-4 flex-shrink-0">
        <p class="text-xs text-gray-500 dark:text-gray-400">
          Tip: You can also type a path directly in the input field.
        </p>
        <button type="button" onclick={closeBrowser} class="btn btn-secondary">
          Cancel
        </button>
      </div>
    </div>
  </div>
{/if}

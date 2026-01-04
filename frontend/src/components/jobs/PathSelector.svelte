<script lang="ts">
  import { api } from '../../lib/api';
  import type { DirectoryEntry } from '../../lib/types';

  let { paths = $bindable<string[]>([]) }: { paths: string[] } = $props();

  let showBrowser = $state(false);
  let currentPath = $state('/');
  let entries = $state<DirectoryEntry[]>([]);
  let loading = $state(false);
  let manualPath = $state('');
  let allowedPaths = $state<string[]>([]);
  let showingRoots = $state(false);

  // Multi-select state
  let selectedInDialog = $state<Set<string>>(new Set());

  // Computed: directories that can be selected (not already in paths)
  let selectableDirectories = $derived(
    entries.filter(e => e.is_dir && !paths.includes(e.path))
  );

  // Computed: count of new selections (not already in paths)
  let newSelectionCount = $derived(
    [...selectedInDialog].filter(p => !paths.includes(p)).length
  );

  async function browse(path: string) {
    loading = true;
    showingRoots = false;
    try {
      const result = await api.system.browse(path);
      currentPath = result.path;
      entries = result.entries;
    } catch {
      entries = [];
    } finally {
      loading = false;
    }
  }

  async function openBrowser() {
    selectedInDialog = new Set();
    showBrowser = true;
    loading = true;

    try {
      // Fetch allowed paths and show them as root options
      const result = await api.system.allowedPaths();
      allowedPaths = result.paths;

      if (allowedPaths.length === 1) {
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
        }));
        loading = false;
      } else {
        // No allowed paths configured
        entries = [];
        loading = false;
      }
    } catch {
      entries = [];
      loading = false;
    }
  }

  function closeBrowser() {
    showBrowser = false;
    selectedInDialog = new Set();
  }

  function toggleSelection(path: string) {
    const newSet = new Set(selectedInDialog);
    if (newSet.has(path)) {
      newSet.delete(path);
    } else {
      newSet.add(path);
    }
    selectedInDialog = newSet;
  }

  function selectAllVisible() {
    const newSet = new Set(selectedInDialog);
    for (const dir of selectableDirectories) {
      newSet.add(dir.path);
    }
    selectedInDialog = newSet;
  }

  function clearSelection() {
    selectedInDialog = new Set();
  }

  function addSelectedPaths() {
    const newPaths = [...selectedInDialog].filter(p => !paths.includes(p));
    if (newPaths.length > 0) {
      paths = [...paths, ...newPaths];
    }
    closeBrowser();
  }

  function quickAddCurrentPath() {
    if (!paths.includes(currentPath)) {
      paths = [...paths, currentPath];
    }
  }

  function removePath(path: string) {
    paths = paths.filter((p) => p !== path);
  }

  function addManualPath() {
    const path = manualPath.trim();
    if (path && !paths.includes(path)) {
      paths = [...paths, path];
      manualPath = '';
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addManualPath();
    }
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
      }));
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
        }));
      }
    }
  }

  function isAlreadyAdded(path: string): boolean {
    return paths.includes(path);
  }
</script>

<div class="space-y-3">
  <!-- Selected Paths -->
  {#if paths.length > 0}
    <div class="space-y-2">
      {#each paths as path}
        <div class="flex items-center justify-between p-2 bg-gray-50 dark:bg-gray-700 rounded-lg">
          <code class="text-sm text-gray-800 dark:text-gray-200">{path}</code>
          <button onclick={() => removePath(path)} class="text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300" aria-label="Remove path">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Add Path -->
  <div class="flex gap-2">
    <input
      type="text"
      bind:value={manualPath}
      onkeydown={handleKeydown}
      placeholder="Enter path manually or browse..."
      class="input flex-1"
    />
    <button type="button" onclick={addManualPath} class="btn btn-secondary">Add</button>
    <button type="button" onclick={openBrowser} class="btn btn-secondary">Browse</button>
  </div>
</div>

<!-- File Browser Modal -->
{#if showBrowser}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    onclick={(e) => e.target === e.currentTarget && closeBrowser()}
    onkeydown={(e) => e.key === 'Escape' && closeBrowser()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col">
      <!-- Header -->
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

      <!-- Navigation & Quick Actions -->
      <div class="p-2 border-b border-gray-200 dark:border-gray-700 flex flex-wrap gap-2 items-center">
        <button type="button" onclick={goUp} disabled={showingRoots || (allowedPaths.includes(currentPath) && allowedPaths.length === 1)} class="btn btn-secondary text-sm">
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 10l7-7m0 0l7 7m-7-7v18" />
          </svg>
          Up
        </button>
        <button
          type="button"
          onclick={quickAddCurrentPath}
          disabled={isAlreadyAdded(currentPath)}
          class="btn btn-secondary text-sm"
          title={isAlreadyAdded(currentPath) ? 'Already added' : 'Add current directory'}
        >
          {#if isAlreadyAdded(currentPath)}
            <svg class="w-4 h-4 mr-1 text-green-500" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
            </svg>
            Added
          {:else}
            <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
            Add This Directory
          {/if}
        </button>
        <div class="flex-1"></div>
        {#if selectableDirectories.length > 0}
          <button type="button" onclick={selectAllVisible} class="text-sm text-primary-600 dark:text-primary-400 hover:underline">
            Select all
          </button>
        {/if}
        {#if selectedInDialog.size > 0}
          <button type="button" onclick={clearSelection} class="text-sm text-gray-500 dark:text-gray-400 hover:underline">
            Clear ({selectedInDialog.size})
          </button>
        {/if}
      </div>

      <!-- Directory Listing -->
      <div class="flex-1 overflow-auto p-2">
        {#if loading}
          <div class="flex justify-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if entries.length === 0}
          <p class="text-center text-gray-500 dark:text-gray-400 py-8">Empty directory</p>
        {:else}
          <div class="space-y-1">
            {#each entries as entry}
              {#if entry.is_dir}
                {@const alreadyAdded = isAlreadyAdded(entry.path)}
                {@const isSelected = selectedInDialog.has(entry.path)}
                <div
                  class="flex items-center gap-2 p-2 rounded transition-colors {alreadyAdded ? 'bg-green-50 dark:bg-green-900/20' : isSelected ? 'bg-primary-50 dark:bg-primary-900/30' : 'hover:bg-gray-100 dark:hover:bg-gray-700'}"
                >
                  <!-- Checkbox for selection -->
                  {#if alreadyAdded}
                    <div class="w-5 h-5 flex items-center justify-center" title="Already added">
                      <svg class="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                      </svg>
                    </div>
                  {:else}
                    <button
                      type="button"
                      onclick={() => toggleSelection(entry.path)}
                      class="w-5 h-5 rounded border-2 flex items-center justify-center transition-colors {isSelected ? 'bg-primary-600 border-primary-600' : 'border-gray-300 dark:border-gray-600 hover:border-primary-500'}"
                      aria-label={isSelected ? 'Deselect directory' : 'Select directory'}
                    >
                      {#if isSelected}
                        <svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                        </svg>
                      {/if}
                    </button>
                  {/if}

                  <!-- Folder icon and name (clickable to navigate) -->
                  <button
                    type="button"
                    onclick={() => browse(entry.path)}
                    class="flex-1 flex items-center gap-2 text-left min-w-0"
                  >
                    <svg class="w-5 h-5 flex-shrink-0 {alreadyAdded ? 'text-green-500' : 'text-yellow-500'}" fill="currentColor" viewBox="0 0 20 20">
                      <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                    </svg>
                    <span class="truncate {alreadyAdded ? 'text-green-700 dark:text-green-400' : 'text-gray-900 dark:text-gray-100'}">{entry.name}</span>
                  </button>

                  <!-- Quick add button for individual directory -->
                  {#if !alreadyAdded && !isSelected}
                    <button
                      type="button"
                      onclick={() => toggleSelection(entry.path)}
                      class="text-xs text-primary-600 dark:text-primary-400 hover:underline flex-shrink-0"
                      title="Add to selection"
                    >
                      +Add
                    </button>
                  {/if}
                </div>
              {:else}
                <div class="flex items-center gap-2 p-2 text-gray-400 pl-9">
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

      <!-- Footer with action buttons -->
      <div class="p-3 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between gap-3">
        <div class="text-sm text-gray-500 dark:text-gray-400">
          {#if newSelectionCount > 0}
            <span class="font-medium text-primary-600 dark:text-primary-400">{newSelectionCount}</span> director{newSelectionCount === 1 ? 'y' : 'ies'} selected
          {:else}
            Select directories to add
          {/if}
        </div>
        <div class="flex gap-2">
          <button type="button" onclick={closeBrowser} class="btn btn-secondary">
            Cancel
          </button>
          <button
            type="button"
            onclick={addSelectedPaths}
            disabled={newSelectionCount === 0}
            class="btn btn-primary"
          >
            {#if newSelectionCount > 0}
              Add {newSelectionCount} Director{newSelectionCount === 1 ? 'y' : 'ies'}
            {:else}
              Add Selected
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

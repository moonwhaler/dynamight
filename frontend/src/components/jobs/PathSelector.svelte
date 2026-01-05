<script lang="ts">
  import { api } from '../../lib/api';
  import type { DirectoryEntry } from '../../lib/types';
  import * as m from '$lib/paraglide/messages.js';

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
          size: null,
          modified: null,
          extension: null,
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
        size: null,
        modified: null,
        extension: null,
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
          size: null,
          modified: null,
          extension: null,
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
    <div class="border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden divide-y divide-gray-200 dark:divide-gray-700">
      {#each paths as path}
        <div class="flex items-center justify-between gap-3 px-3 py-2.5 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-750">
          <div class="flex items-center gap-2.5 min-w-0">
            <svg class="w-5 h-5 flex-shrink-0 text-yellow-500" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
            </svg>
            <code class="text-sm text-gray-700 dark:text-gray-200 truncate">{path}</code>
          </div>
          <button
            onclick={() => removePath(path)}
            class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
            aria-label="Remove path"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
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
      placeholder={m.path_selector_placeholder_manual()}
      class="input flex-1"
    />
    <button type="button" onclick={addManualPath} class="btn btn-secondary">{m.common_add()}</button>
    <button type="button" onclick={openBrowser} class="btn btn-secondary">{m.path_selector_browse()}</button>
  </div>
</div>

<!-- File Browser Modal -->
{#if showBrowser}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4"
    onclick={(e) => e.target === e.currentTarget && closeBrowser()}
    onkeydown={(e) => e.key === 'Escape' && closeBrowser()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl max-w-2xl w-full h-[70vh] flex flex-col overflow-hidden">
      <!-- Header -->
      <div class="px-5 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div class="min-w-0 flex-1">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.path_selector_select_directories()}</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400 font-mono truncate mt-0.5">{currentPath}</p>
        </div>
        <button
          onclick={closeBrowser}
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-xl transition-colors ml-3"
          aria-label={m.common_close()}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Navigation & Quick Actions -->
      <div class="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex flex-wrap gap-2 items-center flex-shrink-0">
        <button
          type="button"
          onclick={goUp}
          disabled={showingRoots || (allowedPaths.includes(currentPath) && allowedPaths.length === 1)}
          class="btn btn-secondary text-sm inline-flex items-center"
        >
          <svg class="w-4 h-4 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 11l5-5m0 0l5 5m-5-5v12" />
          </svg>
          {m.path_selector_up()}
        </button>
        <button
          type="button"
          onclick={quickAddCurrentPath}
          disabled={isAlreadyAdded(currentPath)}
          class="btn {isAlreadyAdded(currentPath) ? 'btn-secondary' : 'btn-primary'} text-sm inline-flex items-center"
          title={isAlreadyAdded(currentPath) ? m.path_selector_already_added() : m.path_selector_add_this_directory()}
        >
          {#if isAlreadyAdded(currentPath)}
            <svg class="w-4 h-4 mr-1.5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
            </svg>
            {m.path_selector_added()}
          {:else}
            <svg class="w-4 h-4 mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
            {m.path_selector_add_this_directory()}
          {/if}
        </button>
        <div class="flex-1"></div>
        {#if selectableDirectories.length > 0}
          <button type="button" onclick={selectAllVisible} class="text-sm text-primary-600 dark:text-primary-400 hover:underline font-medium px-2">
            {m.path_selector_select_all()}
          </button>
        {/if}
        {#if selectedInDialog.size > 0}
          <button type="button" onclick={clearSelection} class="text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 px-2">
            {m.path_selector_clear_selection({ count: selectedInDialog.size })}
          </button>
        {/if}
      </div>

      <!-- Directory Listing -->
      <div class="flex-1 min-h-0 overflow-y-auto p-3">
        {#if loading}
          <div class="flex items-center justify-center h-full">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if entries.length === 0}
          <div class="flex flex-col items-center justify-center h-full text-gray-500 dark:text-gray-400">
            <svg class="w-12 h-12 mb-3 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <p>{m.path_selector_empty_directory()}</p>
          </div>
        {:else}
          <div class="space-y-0.5">
            {#each entries as entry}
              {#if entry.is_dir}
                {@const alreadyAdded = isAlreadyAdded(entry.path)}
                {@const isSelected = selectedInDialog.has(entry.path)}
                <div
                  class="flex items-center gap-3 px-3 py-2.5 rounded-xl transition-colors {alreadyAdded ? 'bg-green-50 dark:bg-green-900/20' : isSelected ? 'bg-primary-50 dark:bg-primary-900/30' : 'hover:bg-gray-100 dark:hover:bg-gray-700/50'}"
                >
                  <!-- Checkbox for selection -->
                  {#if alreadyAdded}
                    <div class="w-5 h-5 flex items-center justify-center flex-shrink-0" title={m.path_selector_already_added()}>
                      <svg class="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                      </svg>
                    </div>
                  {:else}
                    <button
                      type="button"
                      onclick={() => toggleSelection(entry.path)}
                      class="w-5 h-5 rounded border-2 flex items-center justify-center transition-colors flex-shrink-0 {isSelected ? 'bg-primary-600 border-primary-600' : 'border-gray-300 dark:border-gray-600 hover:border-primary-500'}"
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
                    class="flex-1 flex items-center gap-2.5 text-left min-w-0"
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
                      class="text-xs font-medium text-primary-600 dark:text-primary-400 hover:text-primary-700 dark:hover:text-primary-300 flex-shrink-0 px-2 py-1 rounded-lg hover:bg-primary-50 dark:hover:bg-primary-900/30 transition-colors"
                      title={m.path_selector_add_to_selection()}
                    >
                      +{m.common_add()}
                    </button>
                  {/if}
                </div>
              {:else}
                <div class="flex items-center gap-3 px-3 py-2.5 text-gray-400 dark:text-gray-500 ml-8">
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
      <div class="px-5 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex items-center justify-between gap-4 flex-shrink-0">
        <div class="text-sm text-gray-600 dark:text-gray-400">
          {#if newSelectionCount > 0}
            {newSelectionCount === 1 ? m.path_selector_directory_selected({ count: newSelectionCount }) : m.path_selector_directories_selected({ count: newSelectionCount })}
          {:else}
            <span class="text-gray-500 dark:text-gray-500">{m.path_selector_select_to_add()}</span>
          {/if}
        </div>
        <div class="flex gap-3">
          <button type="button" onclick={closeBrowser} class="btn btn-secondary">
            {m.common_cancel()}
          </button>
          <button
            type="button"
            onclick={addSelectedPaths}
            disabled={newSelectionCount === 0}
            class="btn btn-primary"
          >
            {#if newSelectionCount > 0}
              {newSelectionCount === 1 ? m.path_selector_add_directory({ count: newSelectionCount }) : m.path_selector_add_directories({ count: newSelectionCount })}
            {:else}
              {m.path_selector_add_selected()}
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

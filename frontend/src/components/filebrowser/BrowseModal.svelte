<script lang="ts">
  import { api } from '$lib/api';
  import type { DirectoryEntry } from '$lib/types';
  import BreadcrumbNav from './BreadcrumbNav.svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    open: boolean;
    rootPath: string;
    onSelect: (path: string) => void;
    onClose: () => void;
    allowedPaths?: string[];
  }

  let { open = $bindable(), rootPath, onSelect, onClose, allowedPaths = [] }: Props = $props();

  // Local state
  let currentPath = $state('');
  let entries = $state<DirectoryEntry[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let pathHistory = $state<string[]>([]);

  // Watch for open changes to initialize
  $effect(() => {
    if (open && rootPath) {
      browsePath(rootPath);
    }
  });

  async function browsePath(path: string) {
    loading = true;
    error = null;

    try {
      const result = await api.system.browse(path);

      // Store current path in history before navigating
      if (currentPath && currentPath !== path) {
        pathHistory = [...pathHistory, currentPath];
      }

      currentPath = result.path;
      entries = result.entries;
    } catch (e) {
      error = e instanceof Error ? e.message : m.error_generic();
    } finally {
      loading = false;
    }
  }

  function handleNavigate(path: string) {
    browsePath(path);
  }

  function handleGoUp() {
    const isAtRoot = allowedPaths.includes(currentPath) || currentPath === rootPath;
    if (isAtRoot) return;

    const parent = currentPath.split('/').slice(0, -1).join('/') || '/';

    // Check if parent is still within allowed scope
    const isParentAllowed = allowedPaths.length === 0 ||
      allowedPaths.some(allowed => parent.startsWith(allowed) || allowed.startsWith(parent));

    if (isParentAllowed) {
      browsePath(parent);
    }
  }

  function handleGoBack() {
    if (pathHistory.length > 0) {
      const prevPath = pathHistory[pathHistory.length - 1];
      pathHistory = pathHistory.slice(0, -1);
      browsePath(prevPath);
    }
  }

  function handleEntryClick(entry: DirectoryEntry) {
    if (entry.is_dir) {
      browsePath(entry.path);
    }
  }

  function handleSelect() {
    onSelect(currentPath);
    handleClose();
  }

  function handleClose() {
    open = false;
    currentPath = '';
    entries = [];
    pathHistory = [];
    error = null;
    onClose();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      handleClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleClose();
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="presentation"
  >
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl max-w-3xl w-full h-[70vh] flex flex-col overflow-hidden"
      role="dialog"
      aria-modal="true"
      aria-labelledby="browse-modal-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <div class="px-5 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between flex-shrink-0">
        <div class="min-w-0 flex-1">
          <h3 id="browse-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white">
            {m.filebrowser_select_folder()}
          </h3>
          <p class="text-sm text-gray-500 dark:text-gray-400 font-mono truncate mt-0.5">
            {currentPath || rootPath}
          </p>
        </div>
        <button
          onclick={handleClose}
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-xl transition-colors ml-3"
          aria-label={m.common_close()}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Error Message -->
      {#if error}
        <div class="mx-4 mt-3 px-4 py-2.5 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 text-sm rounded-xl flex-shrink-0">
          {error}
        </div>
      {/if}

      <!-- Breadcrumb Navigation -->
      <div class="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex-shrink-0">
        <BreadcrumbNav
          path={currentPath}
          {allowedPaths}
          onNavigate={handleNavigate}
          onGoUp={handleGoUp}
          onGoBack={handleGoBack}
          canGoBack={pathHistory.length > 0}
        />
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
            <p>{m.filebrowser_empty()}</p>
          </div>
        {:else}
          <div class="space-y-0.5">
            {#each entries as entry}
              {#if entry.is_dir}
                <button
                  onclick={() => handleEntryClick(entry)}
                  class="w-full flex items-center gap-2.5 px-3 py-2.5 hover:bg-gray-100 dark:hover:bg-gray-700/50 rounded-xl text-left transition-colors"
                >
                  <svg class="w-5 h-5 text-yellow-500 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                  </svg>
                  <span class="text-gray-900 dark:text-gray-100 truncate font-medium">{entry.name}</span>
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
      <div class="px-5 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex items-center justify-end gap-3 flex-shrink-0">
        <button type="button" onclick={handleClose} class="btn btn-secondary">
          {m.common_cancel()}
        </button>
        <button
          type="button"
          onclick={handleSelect}
          disabled={!currentPath}
          class="btn btn-primary"
        >
          {m.filebrowser_select_folder()}
        </button>
      </div>
    </div>
  </div>
{/if}

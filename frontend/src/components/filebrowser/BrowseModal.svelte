<script lang="ts">
  import { api } from '$lib/api';
  import type { DirectoryEntry } from '$lib/types';
  import BreadcrumbNav from './BreadcrumbNav.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Spinner from '../ui/Spinner.svelte';
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

  function reset() {
    currentPath = '';
    entries = [];
    pathHistory = [];
    error = null;
    onClose();
  }

  function handleClose() {
    open = false;
    reset();
  }
</script>

<Dialog bind:open title={m.filebrowser_select_folder()} size="lg" onclose={reset} bodyClass="p-0 flex flex-col h-[60dvh] sm:h-[32rem]">
  {#snippet header()}
    <span class="hidden sm:block max-w-[16rem] truncate font-mono text-xs text-gray-500 dark:text-gray-400" title={currentPath || rootPath}>
      {currentPath || rootPath}
    </span>
  {/snippet}

  {#if error}
    <div class="mx-4 mt-3 px-3 py-2 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm rounded-lg">{error}</div>
  {/if}

  <div class="px-3 sm:px-4 py-2 border-b border-gray-200 dark:border-gray-700 shrink-0">
    <BreadcrumbNav
      path={currentPath}
      {allowedPaths}
      onNavigate={handleNavigate}
      onGoUp={handleGoUp}
      onGoBack={handleGoBack}
      canGoBack={pathHistory.length > 0}
    />
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-2">
    {#if loading}
      <div class="flex items-center justify-center h-full"><Spinner size="w-7 h-7" /></div>
    {:else if entries.length === 0}
      <EmptyState icon="folder" title={m.filebrowser_empty()} compact />
    {:else}
      <ul class="space-y-0.5">
        {#each entries as entry (entry.path)}
          <li>
            {#if entry.is_dir}
              <button
                type="button"
                onclick={() => handleEntryClick(entry)}
                class="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg text-left hover:bg-gray-100 dark:hover:bg-gray-700/50 active:bg-gray-200 dark:active:bg-gray-700"
              >
                <svg class="w-5 h-5 text-amber-500 shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                  <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                </svg>
                <span class="text-sm text-gray-900 dark:text-gray-100 truncate font-medium">{entry.name}</span>
              </button>
            {:else}
              <div class="flex items-center gap-2.5 px-3 py-2.5 text-gray-400 dark:text-gray-500">
                <svg class="w-5 h-5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.6" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                </svg>
                <span class="text-sm truncate">{entry.name}</span>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#snippet footer()}
    <button type="button" onclick={handleClose} class="btn btn-secondary">{m.common_cancel()}</button>
    <button type="button" onclick={handleSelect} disabled={!currentPath} class="btn btn-primary">
      {m.filebrowser_select_folder()}
    </button>
  {/snippet}
</Dialog>

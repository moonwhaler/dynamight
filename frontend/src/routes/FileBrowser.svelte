<script lang="ts">
  import { onMount } from 'svelte';
  import { fileBrowserStore } from '$lib/stores/fileBrowser';
  import { fileBrowserTablePreferencesStore, FB_ALL, FB_FIXED, FB_DEFAULT_VISIBLE } from '$lib/stores/fileBrowserTablePreferences';
  import type { FileBrowserColumnKey } from '$lib/stores/fileBrowserTablePreferences';
  import type { UsbDrive, SearchMode, DirectoryEntry } from '$lib/types';
  import { api } from '$lib/api';
  import BreadcrumbNav from '../components/filebrowser/BreadcrumbNav.svelte';
  import FileList from '../components/filebrowser/FileList.svelte';
  import FileSearchBar from '../components/filebrowser/FileSearchBar.svelte';
  import DriveSelector from '../components/filebrowser/DriveSelector.svelte';
  import DeleteConfirmDialog from '../components/filebrowser/DeleteConfirmDialog.svelte';
  import ColumnSelector from '../components/ui/ColumnSelector.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // Subscribe to store
  let browserState = $derived($fileBrowserStore);

  // Search state
  let searchQuery = $state('');
  let isSearchOpen = $state(false);
  let searchMode: SearchMode = $state('local');
  let deepSearchResults: DirectoryEntry[] = $state([]);
  let deepSearchLoading = $state(false);
  let deepSearchTruncated = $state(false);
  let deepSearchTimedOut = $state(false);
  let deepSearchBasePath = $state('');
  // These must NOT be $state — reading them inside functions called during
  // async work would create reactive dependencies and cause infinite loops.
  let abortController: AbortController | null = null;

  function abortDeepSearch() {
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
  }

  async function performDeepSearch() {
    const q = searchQuery.trim();
    const path = browserState.currentPath;

    if (searchMode !== 'deep' || q.length < 2 || !path) return;

    abortDeepSearch();
    deepSearchLoading = true;
    deepSearchTimedOut = false;

    const controller = new AbortController();
    abortController = controller;
    try {
      const res = await api.system.search(path, q, 200, { signal: controller.signal });
      if (!controller.signal.aborted) {
        deepSearchResults = res.results;
        deepSearchTruncated = res.truncated;
        deepSearchTimedOut = res.timed_out;
        deepSearchBasePath = res.base_path;
      }
    } catch (e: unknown) {
      if (e instanceof DOMException && e.name === 'AbortError') return;
      if (!controller.signal.aborted) {
        deepSearchResults = [];
        deepSearchTruncated = false;
        deepSearchTimedOut = false;
        deepSearchBasePath = '';
      }
    } finally {
      if (!controller.signal.aborted) {
        deepSearchLoading = false;
      }
    }
  }

  function sortResults(entries: DirectoryEntry[]): DirectoryEntry[] {
    const { sortBy, sortOrder } = browserState;
    return [...entries].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      switch (sortBy) {
        case 'name':     cmp = a.name.toLowerCase().localeCompare(b.name.toLowerCase()); break;
        case 'size':     cmp = (a.size ?? 0) - (b.size ?? 0); break;
        case 'modified': cmp = (a.modified ?? 0) - (b.modified ?? 0); break;
      }
      return sortOrder === 'asc' ? cmp : -cmp;
    });
  }

  const filteredEntries = $derived.by(() => {
    if (searchMode === 'deep' && searchQuery.trim().length >= 2) {
      return sortResults(deepSearchResults);
    }
    const q = searchQuery.trim().toLowerCase();
    if (!q) return browserState.entries;
    return browserState.entries.filter(e => e.name.toLowerCase().includes(q));
  });

  // New folder dialog state
  let showNewFolderDialog = $state(false);
  let newFolderName = $state('');
  let newFolderError = $state('');

  // Delete dialog state
  let showDeleteDialog = $state(false);
  let pendingDeletePath = $state('');
  let pendingDeleteName = $state('');
  let pendingDeleteIsDir = $state(false);

  onMount(() => {
    // Load drives and allowed paths
    fileBrowserStore.loadDrives();
    fileBrowserStore.loadAllowedPaths();

    function handleGlobalKeydown(e: KeyboardEvent) {
      const target = e.target as HTMLElement;
      const inInput = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target.isContentEditable;

      if (e.key === '/' && !inInput && browserState.currentPath) {
        e.preventDefault();
        openSearch();
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === 'f' && browserState.currentPath) {
        e.preventDefault();
        openSearch();
        return;
      }

      if (e.key === 'Escape' && isSearchOpen && !showNewFolderDialog) {
        closeSearch();
      }
    }

    document.addEventListener('keydown', handleGlobalKeydown);
    return () => document.removeEventListener('keydown', handleGlobalKeydown);
  });

  // Search helpers
  function openSearch() {
    isSearchOpen = true;
  }

  function closeSearch() {
    abortDeepSearch();
    isSearchOpen = false;
    searchQuery = '';
    searchMode = 'local';
    deepSearchResults = [];
    deepSearchLoading = false;
    deepSearchTruncated = false;
    deepSearchTimedOut = false;
    deepSearchBasePath = '';
  }

  function handleSearchModeChange(mode: SearchMode) {
    searchMode = mode;
    if (mode === 'local') {
      abortDeepSearch();
      deepSearchResults = [];
      deepSearchLoading = false;
      deepSearchTruncated = false;
      deepSearchTimedOut = false;
      deepSearchBasePath = '';
    }
  }

  // Navigation handlers
  function handleNavigate(path: string) {
    // In deep search mode, clicking a file navigates to its parent directory
    if (searchMode === 'deep') {
      // Check if this is a file (not a directory in the deep results)
      const entry = deepSearchResults.find(e => e.path === path);
      if (entry && !entry.is_dir) {
        const parentDir = path.split('/').slice(0, -1).join('/');
        closeSearch();
        fileBrowserStore.browsePath(parentDir);
        return;
      }
      // Directory: navigate into it and close search
      closeSearch();
    }
    fileBrowserStore.browsePath(path);
  }

  function handleGoUp() {
    fileBrowserStore.goUp();
  }

  function handleGoBack() {
    fileBrowserStore.goBack();
  }

  function handleDownload(path: string) {
    fileBrowserStore.downloadFile(path);
  }

  // Delete handlers
  async function handleDelete(path: string, name: string, isDir: boolean) {
    // Check if already verified
    if (fileBrowserStore.isDeleteVerified()) {
      // Already verified, proceed with delete directly
      await fileBrowserStore.deleteEntry(path);
    } else {
      // Need verification, show dialog
      pendingDeletePath = path;
      pendingDeleteName = name;
      pendingDeleteIsDir = isDir;
      showDeleteDialog = true;
    }
  }

  function closeDeleteDialog() {
    showDeleteDialog = false;
    pendingDeletePath = '';
    pendingDeleteName = '';
    pendingDeleteIsDir = false;
  }

  function handleDeleteSuccess() {
    closeDeleteDialog();
  }

  // Drive handlers
  async function handleBrowseDrive(drive: UsbDrive) {
    if (drive.mountpoint) {
      fileBrowserStore.browsePath(drive.mountpoint);
    }
  }

  async function handleMount(drive: UsbDrive) {
    // Generate mount point and mount
    const mountPoint = await fileBrowserStore.generateMountPoint(drive.uuid, drive.label ?? undefined);
    if (mountPoint) {
      const success = await fileBrowserStore.mountDrive(drive.uuid, mountPoint);
      if (success) {
        // Browse to the newly mounted drive
        fileBrowserStore.browsePath(mountPoint);
      }
    }
  }

  async function handleUnmount(drive: UsbDrive) {
    if (drive.mountpoint) {
      await fileBrowserStore.unmountDrive(drive.mountpoint);
    }
  }

  function handleRefreshDrives() {
    fileBrowserStore.loadDrives();
  }

  // View mode toggle
  function toggleViewMode() {
    fileBrowserStore.setViewMode(browserState.viewMode === 'list' ? 'grid' : 'list');
  }

  // Sort handlers
  function handleSortChange(field: 'name' | 'size' | 'modified') {
    fileBrowserStore.setSortBy(field);
  }

  function handleSortOrderToggle() {
    fileBrowserStore.toggleSortOrder();
  }

  function columnLabel(col: string): string {
    switch (col as FileBrowserColumnKey) {
      case 'name':     return m.filebrowser_column_name();
      case 'size':     return m.filebrowser_column_size();
      case 'modified': return m.filebrowser_column_modified();
      case 'actions':  return m.filebrowser_column_actions();
      default:         return col;
    }
  }

  function handleColumnToggle(col: string) {
    const key = col as FileBrowserColumnKey;
    fileBrowserTablePreferencesStore.setColumnVisibility(
      key,
      !$fileBrowserTablePreferencesStore.visibleColumns.includes(key)
    );
  }

  // New folder handlers
  function openNewFolderDialog() {
    newFolderName = '';
    newFolderError = '';
    showNewFolderDialog = true;
  }

  function closeNewFolderDialog() {
    showNewFolderDialog = false;
    newFolderName = '';
    newFolderError = '';
  }

  async function createNewFolder() {
    if (!newFolderName.trim()) {
      newFolderError = m.error_field_required({ field: 'Name' });
      return;
    }

    // Validate folder name
    if (newFolderName.includes('/') || newFolderName.includes('..')) {
      newFolderError = m.error_path_not_allowed();
      return;
    }

    const newPath = `${browserState.currentPath}/${newFolderName}`.replace(/\/+/g, '/');
    const success = await fileBrowserStore.createFolder(newPath);

    if (success) {
      closeNewFolderDialog();
    } else {
      newFolderError = browserState.error || m.error_generic();
    }
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{m.filebrowser_title()}</h1>
    <div class="flex items-center gap-2">
      <!-- View mode toggle -->
      <button
        type="button"
        onclick={toggleViewMode}
        class="py-2.5 px-2 rounded-lg border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-600 dark:text-gray-300"
        title={browserState.viewMode === 'list' ? 'Grid view' : 'List view'}
      >
        {#if browserState.viewMode === 'list'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
          </svg>
        {:else}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
          </svg>
        {/if}
      </button>

      <!-- Search toggle (only when browsing) -->
      {#if browserState.currentPath}
        <button
          type="button"
          onclick={() => isSearchOpen ? closeSearch() : openSearch()}
          class="py-2.5 px-2 rounded-lg border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-600 dark:text-gray-300
            {isSearchOpen ? 'bg-primary-50 dark:bg-primary-900/20 !text-primary-600 dark:!text-primary-400 border-primary-200 dark:border-primary-800' : ''}"
          title={m.filebrowser_search_toggle()}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </button>
      {/if}

      <!-- Column selector (only in list mode when browsing) -->
      {#if browserState.viewMode === 'list' && browserState.currentPath}
        <ColumnSelector
          visibleColumns={$fileBrowserTablePreferencesStore.visibleColumns}
          allColumns={FB_ALL}
          fixedColumns={FB_FIXED}
          defaultVisible={FB_DEFAULT_VISIBLE}
          {columnLabel}
          onToggle={handleColumnToggle}
          onReset={() => fileBrowserTablePreferencesStore.reset()}
        />
      {/if}

      <!-- New folder button (only when browsing a path) -->
      {#if browserState.currentPath}
        <button
          type="button"
          onclick={openNewFolderDialog}
          class="btn btn-secondary flex items-center gap-2"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
          </svg>
          <span class="hidden sm:inline">{m.filebrowser_new_folder()}</span>
        </button>
      {/if}
    </div>
  </div>

  <!-- Drive & Path Selector -->
  <div class="card p-4">
    <DriveSelector
      drives={browserState.drives}
      allowedPaths={browserState.allowedPaths}
      loadingDrives={browserState.loadingDrives}
      currentPath={browserState.currentPath}
      onBrowseDrive={handleBrowseDrive}
      onBrowsePath={handleNavigate}
      onMount={handleMount}
      onUnmount={handleUnmount}
      onRefresh={handleRefreshDrives}
    />
  </div>

  <!-- Breadcrumb Navigation (only when browsing) -->
  {#if browserState.currentPath}
    <div class="card px-4 py-2">
      <BreadcrumbNav
        path={browserState.currentPath}
        allowedPaths={browserState.allowedPaths}
        onNavigate={handleNavigate}
        onGoUp={handleGoUp}
        onGoBack={handleGoBack}
        canGoBack={browserState.pathHistory.length > 0}
      />
    </div>
  {/if}

  <!-- File List -->
  {#if browserState.currentPath}
    <div class="card overflow-hidden">
      {#if isSearchOpen}
        <FileSearchBar
          bind:query={searchQuery}
          matchCount={filteredEntries.length}
          totalCount={browserState.entries.length}
          onClose={closeSearch}
          {searchMode}
          onSearchModeChange={handleSearchModeChange}
          onSearch={performDeepSearch}
          {deepSearchLoading}
          {deepSearchTruncated}
          {deepSearchTimedOut}
          deepSearchCount={deepSearchResults.length}
        />
      {/if}
      <FileList
        entries={filteredEntries}
        searchQuery={searchQuery}
        viewMode={browserState.viewMode}
        sortBy={browserState.sortBy}
        sortOrder={browserState.sortOrder}
        loading={browserState.loading}
        downloading={browserState.downloading}
        deleting={browserState.deleting}
        error={browserState.error}
        onNavigate={handleNavigate}
        onDownload={handleDownload}
        onDelete={handleDelete}
        onSortChange={handleSortChange}
        onSortOrderToggle={handleSortOrderToggle}
        clickableFiles={searchMode === 'deep'}
        basePath={deepSearchBasePath}
        {deepSearchLoading}
      />
    </div>
  {:else}
    <!-- Empty state when no path selected -->
    <div class="card p-12 text-center">
      <div class="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
        <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
        </svg>
      </div>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{m.filebrowser_select_location()}</h3>
      <p class="text-gray-500 dark:text-gray-400">{m.filebrowser_select_location_desc()}</p>
    </div>
  {/if}
</div>

<!-- New Folder Dialog -->
{#if showNewFolderDialog}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50"
    onclick={closeNewFolderDialog}
    onkeydown={(e) => e.key === 'Escape' && closeNewFolderDialog()}
    role="presentation"
  >
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="card p-6 w-full max-w-md"
      role="dialog"
      aria-modal="true"
      aria-labelledby="new-folder-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h3 id="new-folder-title" class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{m.filebrowser_new_folder()}</h3>

      <div class="space-y-4">
        <div>
          <label for="folder-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {m.filebrowser_folder_name()}
          </label>
          <input
            type="text"
            id="folder-name"
            bind:value={newFolderName}
            class="input"
            placeholder="New Folder"
            onkeydown={(e) => e.key === 'Enter' && createNewFolder()}
          />
          {#if newFolderError}
            <p class="mt-1 text-sm text-red-500">{newFolderError}</p>
          {/if}
        </div>

        <div class="flex justify-end gap-2">
          <button type="button" class="btn btn-secondary" onclick={closeNewFolderDialog}>
            {m.common_cancel()}
          </button>
          <button type="button" class="btn btn-primary" onclick={createNewFolder}>
            {m.common_create()}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Delete Confirmation Dialog -->
<DeleteConfirmDialog
  open={showDeleteDialog}
  entryName={pendingDeleteName}
  entryPath={pendingDeletePath}
  isDirectory={pendingDeleteIsDir}
  onClose={closeDeleteDialog}
  onSuccess={handleDeleteSuccess}
/>

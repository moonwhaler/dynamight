<script lang="ts">
  import { onMount } from 'svelte';
  import { fileBrowserStore, type SortField } from '$lib/stores/fileBrowser';
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
  import PageHeader from '../components/ui/PageHeader.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import EmptyState from '../components/ui/EmptyState.svelte';
  import Icon from '../components/ui/Icon.svelte';
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
        case 'type':     cmp = (a.extension ?? '').toLowerCase().localeCompare((b.extension ?? '').toLowerCase()); break;
        case 'modified': cmp = (a.modified ?? 0) - (b.modified ?? 0); break;
      }
      return sortOrder === 'asc' ? cmp : -cmp;
    });
  }

  const filteredEntries = $derived.by(() => {
    let base: DirectoryEntry[];
    if (searchMode === 'deep' && searchQuery.trim().length >= 2) {
      base = deepSearchResults;
    } else {
      const q = searchQuery.trim().toLowerCase();
      base = !q ? browserState.entries : browserState.entries.filter(e => e.name.toLowerCase().includes(q));
    }
    return sortResults(base);
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

  // With a single place to browse, open it right away instead of asking
  let autoOpened = false;
  $effect(() => {
    if (autoOpened || browserState.currentPath || browserState.loadingDrives || browserState.loadingAllowedPaths) return;
    const places = [
      ...browserState.allowedPaths,
      ...browserState.drives.filter((d) => d.mountpoint).map((d) => d.mountpoint as string),
    ];
    autoOpened = true;
    if (places.length === 1) handleNavigate(places[0]);
  });

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
  function handleSortChange(field: SortField) {
    fileBrowserStore.setSortBy(field);
  }

  function handleSortOrderToggle() {
    fileBrowserStore.toggleSortOrder();
  }

  function columnLabel(col: string): string {
    switch (col as FileBrowserColumnKey) {
      case 'name':     return m.filebrowser_column_name();
      case 'size':     return m.filebrowser_column_size();
      case 'type':     return m.filebrowser_column_type();
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

<PageHeader title={m.nav_files()} subtitle={m.files_subtitle()}>
  {#snippet actions()}
    {#if browserState.currentPath}
      <button
        type="button"
        onclick={() => (isSearchOpen ? closeSearch() : openSearch())}
        class="btn btn-icon {isSearchOpen ? 'btn-secondary ring-2 ring-primary-500/40' : 'btn-secondary'}"
        aria-pressed={isSearchOpen}
        aria-label={m.filebrowser_search_toggle()}
        title={m.filebrowser_search_toggle()}
      >
        <Icon name="search" />
      </button>
    {/if}
    <button
      type="button"
      onclick={toggleViewMode}
      class="btn btn-secondary btn-icon"
      aria-label={browserState.viewMode === 'list' ? m.jobs_view_grid() : m.jobs_view_list()}
      title={browserState.viewMode === 'list' ? m.jobs_view_grid() : m.jobs_view_list()}
    >
      <Icon name={browserState.viewMode === 'list' ? 'grid' : 'list'} />
    </button>
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
    {#if browserState.currentPath}
      <button type="button" onclick={openNewFolderDialog} class="btn btn-secondary">
        <Icon name="plus" class="w-4 h-4" />
        <span class="hidden sm:inline">{m.filebrowser_new_folder()}</span>
      </button>
    {/if}
  {/snippet}
</PageHeader>

<div class="space-y-4">
  <!-- Drive & Path Selector -->
  <div class="card p-4">
    <DriveSelector
      drives={browserState.drives}
      allowedPaths={browserState.allowedPaths}
      loadingDrives={browserState.loadingDrives}
      loadingAllowedPaths={browserState.loadingAllowedPaths}
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
        loadingDirSizes={browserState.loadingDirSizes}
      />
    </div>
  {:else}
    <div class="card">
      <EmptyState icon="folder-open" title={m.filebrowser_select_location()} text={m.filebrowser_select_location_desc()} />
    </div>
  {/if}
</div>

<!-- New Folder Dialog -->
<Dialog
  bind:open={() => showNewFolderDialog, (v) => { if (!v) closeNewFolderDialog(); }}
  title={m.filebrowser_new_folder()}
  description={browserState.currentPath}
  size="sm"
>
  <form
    id="new-folder-form"
    onsubmit={(e) => {
      e.preventDefault();
      createNewFolder();
    }}
  >
    <label for="folder-name" class="label">{m.filebrowser_folder_name()}</label>
    <input type="text" id="folder-name" bind:value={newFolderName} class="input" autocomplete="off" />
    {#if newFolderError}<p class="mt-1.5 text-sm text-red-600 dark:text-red-400" role="alert">{newFolderError}</p>{/if}
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-secondary" onclick={closeNewFolderDialog}>{m.common_cancel()}</button>
    <button type="submit" form="new-folder-form" class="btn btn-primary">{m.common_create()}</button>
  {/snippet}
</Dialog>

<!-- Delete Confirmation Dialog -->
<DeleteConfirmDialog
  open={showDeleteDialog}
  entryName={pendingDeleteName}
  entryPath={pendingDeletePath}
  isDirectory={pendingDeleteIsDir}
  onClose={closeDeleteDialog}
  onSuccess={handleDeleteSuccess}
/>

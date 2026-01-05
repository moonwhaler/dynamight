<script lang="ts">
  import { onMount } from 'svelte';
  import { fileBrowserStore } from '$lib/stores/fileBrowser';
  import type { UsbDrive } from '$lib/types';
  import BreadcrumbNav from '../components/filebrowser/BreadcrumbNav.svelte';
  import FileList from '../components/filebrowser/FileList.svelte';
  import DriveSelector from '../components/filebrowser/DriveSelector.svelte';
  import * as m from '$lib/paraglide/messages.js';

  // Subscribe to store
  let browserState = $derived($fileBrowserStore);

  // New folder dialog state
  let showNewFolderDialog = $state(false);
  let newFolderName = $state('');
  let newFolderError = $state('');

  onMount(() => {
    // Load drives and allowed paths
    fileBrowserStore.loadDrives();
    fileBrowserStore.loadAllowedPaths();
  });

  // Navigation handlers
  function handleNavigate(path: string) {
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
        class="p-2 rounded-lg border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-600 dark:text-gray-300"
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
    <div class="card">
      <FileList
        entries={browserState.entries}
        viewMode={browserState.viewMode}
        sortBy={browserState.sortBy}
        sortOrder={browserState.sortOrder}
        loading={browserState.loading}
        downloading={browserState.downloading}
        error={browserState.error}
        onNavigate={handleNavigate}
        onDownload={handleDownload}
        onSortChange={handleSortChange}
        onSortOrderToggle={handleSortOrderToggle}
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

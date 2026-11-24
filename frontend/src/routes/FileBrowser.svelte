<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { push, replace, querystring } from 'svelte-spa-router';
  import { api } from '$lib/api';
  import { fileBrowser, parentOf } from '$lib/files/browser.svelte';
  import { sortEntries } from '$lib/files/fileSort';
  import { fileTasks } from '$lib/files/tasks.svelte';
  import { uploader, filesFromDrop, filesFromFolderInput } from '$lib/files/uploader.svelte';
  import { previewKind, audioQueue } from '$lib/files/previewKind';
  import type { DirectoryEntry, FileDrive, FileTaskKind, SearchMode, TrashItem } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import { getDriveFullName } from '$lib/utils/driveUtils';
  import PlacesPanel from '../components/filebrowser/PlacesPanel.svelte';
  import PathBar from '../components/filebrowser/PathBar.svelte';
  import FileList from '../components/filebrowser/FileList.svelte';
  import FileSearchBar from '../components/filebrowser/FileSearchBar.svelte';
  import TrashView from '../components/filebrowser/TrashView.svelte';
  import TrashDialog, { type DeleteTarget } from '../components/filebrowser/TrashDialog.svelte';
  import FilePreview from '../components/filebrowser/FilePreview.svelte';
  import MusicPlayer from '../components/filebrowser/MusicPlayer.svelte';
  import FileTasks from '../components/filebrowser/FileTasks.svelte';
  import ConflictDialog from '../components/filebrowser/ConflictDialog.svelte';
  import UploadConflictDialog from '../components/filebrowser/UploadConflictDialog.svelte';
  import BrowseModal from '../components/filebrowser/BrowseModal.svelte';
  import PageHeader from '../components/ui/PageHeader.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import Menu, { type MenuItem } from '../components/ui/Menu.svelte';
  import { showToast } from '../components/ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  const fb = fileBrowser;

  // ── Location (the URL is the source of truth) ──

  const params = $derived(new URLSearchParams($querystring ?? ''));
  const urlPath = $derived(params.get('path') ?? '');
  const trashView = $derived(params.get('view') === 'trash');

  function go(path: string) {
    push(`/files?path=${encodeURIComponent(path)}`);
  }

  function openTrash() {
    placesSheet = false;
    push('/files?view=trash');
  }

  let firstLoad = true;
  $effect(() => {
    const path = urlPath;
    const trash = trashView;
    untrack(() => {
      closeSearch();
      selected.clear();
      preview = null;
      if (trash) return;
      if (!path) {
        // Back on the page: continue where the user was
        if (fb.path) replace(`/files?path=${encodeURIComponent(fb.path)}`);
        return;
      }
      if (firstLoad || path !== fb.path || fb.error) fb.open(path);
      firstLoad = false;
    });
  });

  // With a single place, open it instead of asking
  $effect(() => {
    if (urlPath || trashView || fb.path || !fb.places) return;
    const all = [
      ...fb.places.places.map((p) => p.path),
      ...(fb.places.drives.map((d) => d.mountpoint).filter(Boolean) as string[]),
    ];
    if (all.length === 1) untrack(() => replace(`/files?path=${encodeURIComponent(all[0])}`));
  });

  const atRoot = $derived(!fb.path || fb.path === fb.root);

  function goUp() {
    if (!atRoot) go(parentOf(fb.path));
  }

  // ── Search ──

  let searchOpen = $state(false);
  let searchQuery = $state('');
  let searchMode = $state<SearchMode>('local');
  let deepResults = $state<DirectoryEntry[]>([]);
  let deepLoading = $state(false);
  let deepTruncated = $state(false);
  let deepTimedOut = $state(false);
  let deepBase = $state('');
  // Not reactive: read inside async work
  let searchAbort: AbortController | null = null;

  function resetDeep() {
    searchAbort?.abort();
    searchAbort = null;
    deepResults = [];
    deepLoading = false;
    deepTruncated = false;
    deepTimedOut = false;
    deepBase = '';
  }

  function closeSearch() {
    resetDeep();
    searchOpen = false;
    searchQuery = '';
    searchMode = 'local';
  }

  function setSearchMode(mode: SearchMode) {
    searchMode = mode;
    if (mode === 'local') resetDeep();
  }

  async function deepSearch() {
    const q = searchQuery.trim();
    if (searchMode !== 'deep' || q.length < 2 || !fb.path) return;
    resetDeep();
    deepLoading = true;
    const controller = new AbortController();
    searchAbort = controller;
    try {
      const res = await api.system.search(fb.path, q, 200, { signal: controller.signal });
      if (controller.signal.aborted) return;
      deepResults = res.results;
      deepTruncated = res.truncated;
      deepTimedOut = res.timed_out;
      deepBase = res.base_path;
    } catch {
      // Aborted or failed: no results
    } finally {
      if (!controller.signal.aborted) deepLoading = false;
    }
  }

  const deepActive = $derived(searchMode === 'deep' && searchQuery.trim().length >= 2);
  const shown = $derived.by(() => {
    if (deepActive) return sortEntries(deepResults, fb.prefs.sortBy, fb.prefs.sortOrder);
    const q = searchQuery.trim().toLowerCase();
    return q ? fb.sorted.filter((e) => e.name.toLowerCase().includes(q)) : fb.sorted;
  });

  // ── Actions ──

  async function download(entry: DirectoryEntry) {
    try {
      await api.files.probe(entry.path);
      const link = document.createElement('a');
      link.href = api.files.rawUrl(entry.path);
      link.download = entry.name;
      document.body.appendChild(link);
      link.click();
      link.remove();
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_download_failed(), variant: 'error' });
    }
  }

  async function copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      showToast({ message: m.filebrowser_path_copied(), variant: 'success', duration: 2500 });
    } catch {
      // No clipboard over plain http: show the path to copy by hand
      showToast({ message: path, variant: 'info', duration: 10000 });
    }
  }

  function open(entry: DirectoryEntry) {
    if (deepActive) {
      // A search hit: go to it (folder) or to the folder holding it (file)
      go(entry.is_dir ? entry.path : parentOf(entry.path));
      return;
    }
    if (entry.is_dir) go(entry.path);
    else showPreview(entry);
  }

  // ── Preview and music ──

  let preview = $state<string | null>(null);
  const previewEntry = $derived(preview ? (shown.find((e) => e.path === preview) ?? null) : null);
  const previewFiles = $derived(shown.filter((e) => !e.is_dir));
  const previewIndex = $derived(previewEntry ? previewFiles.findIndex((e) => e.path === previewEntry.path) : -1);
  let playerQueue = $state<DirectoryEntry[]>([]);
  let playerIndex = $state(0);
  const playingPath = $derived(playerQueue[playerIndex]?.path ?? null);

  function showPreview(entry: DirectoryEntry) {
    preview = entry.path;
    if (previewKind(entry) === 'audio' && playingPath !== entry.path) play(entry);
  }

  function stepPreview(delta: number) {
    const next = previewFiles[previewIndex + delta];
    if (next) preview = next.path;
  }

  function play(entry: DirectoryEntry) {
    const queue = audioQueue(shown);
    playerQueue = queue;
    playerIndex = Math.max(0, queue.findIndex((e) => e.path === entry.path));
  }

  // ── Selection ──

  const selected = new SvelteSet<string>();
  const selectedEntries = $derived(shown.filter((e) => selected.has(e.path)));
  const selectedBytes = $derived(selectedEntries.reduce((sum, e) => sum + (e.size ?? 0), 0));

  /** The selection, or the given entry when nothing is selected */
  function targetsFor(entry?: DirectoryEntry): DirectoryEntry[] {
    if (selected.size > 0 && (!entry || selected.has(entry.path))) return selectedEntries;
    return entry ? [entry] : [];
  }

  async function downloadMany(entries: DirectoryEntry[]) {
    if (entries.length === 1 && !entries[0].is_dir) return download(entries[0]);
    try {
      const { url, filename } = await api.files.archive(entries.map((e) => e.path));
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_download_failed(), variant: 'error' });
    }
  }

  // ── Copy and move ──

  let pickerOpen = $state(false);
  let pickerKind = $state<FileTaskKind>('copy');
  let pickerSources = $state<string[]>([]);

  function askTransfer(kind: FileTaskKind, entries: DirectoryEntry[]) {
    pickerKind = kind;
    pickerSources = entries.map((e) => e.path);
    pickerOpen = true;
  }

  async function transfer(target: string) {
    try {
      await fileTasks.start(pickerKind, pickerSources, target);
      selected.clear();
      showToast({
        message: pickerKind === 'copy' ? m.filebrowser_copy_started() : m.filebrowser_move_started(),
        variant: 'info',
        duration: 3000,
      });
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_file_operation_failed(), variant: 'error' });
    }
  }

  // ── Upload ──

  let fileInput = $state<HTMLInputElement | null>(null);
  let folderInput = $state<HTMLInputElement | null>(null);
  let dragDepth = $state(0);

  function upload(files: { file: File; relDir: string }[]) {
    if (!fb.path || files.length === 0) return;
    uploader.add(files, fb.path);
  }

  function hasFiles(e: DragEvent): boolean {
    return !!e.dataTransfer?.types.includes('Files');
  }

  async function onDrop(e: DragEvent) {
    if (!hasFiles(e) || !e.dataTransfer) return;
    e.preventDefault();
    dragDepth = 0;
    try {
      upload(await filesFromDrop(e.dataTransfer));
    } catch {
      showToast({ message: m.filebrowser_upload_read_failed(), variant: 'error' });
    }
  }

  const uploadItems = $derived<MenuItem[]>([
    { label: m.filebrowser_upload_files(), icon: 'file', onclick: () => fileInput?.click() },
    { label: m.filebrowser_upload_folder(), icon: 'folder', onclick: () => folderInput?.click() },
  ]);

  // A summary once all uploads of a batch are through
  let uploadsWereOpen = false;
  $effect(() => {
    const open = uploader.open.length > 0;
    if (uploadsWereOpen && !open) {
      const done = uploader.items.filter((i) => i.state === 'done').length;
      const failed = uploader.items.filter((i) => i.state === 'failed').length;
      untrack(() =>
        showToast(
          failed > 0
            ? { message: m.filebrowser_uploads_failed({ count: failed }), variant: 'error' }
            : { message: m.filebrowser_uploads_done({ count: done }), variant: 'success' },
        ),
      );
    }
    uploadsWereOpen = open;
  });

  // ── Rename ──

  let renameOpen = $state(false);
  let renameEntry = $state<DirectoryEntry | null>(null);
  let renameValue = $state('');
  let renameError = $state('');
  let renameInput = $state<HTMLInputElement | null>(null);

  function askRename(entry: DirectoryEntry) {
    renameEntry = entry;
    renameValue = entry.name;
    renameError = '';
    renameOpen = true;
    // Select the name without its extension
    setTimeout(() => {
      const dot = entry.is_dir ? -1 : entry.name.lastIndexOf('.');
      renameInput?.focus();
      renameInput?.setSelectionRange(0, dot > 0 ? dot : entry.name.length);
    }, 50);
  }

  async function doRename(e: Event) {
    e.preventDefault();
    if (!renameEntry) return;
    const name = renameValue.trim();
    if (!name || name === renameEntry.name) {
      renameOpen = false;
      return;
    }
    try {
      const { path } = await api.files.rename(renameEntry.path, name);
      const was = renameEntry.path;
      renameOpen = false;
      await fb.refresh();
      if (preview === was) preview = path;
    } catch (err) {
      renameError = err instanceof Error ? err.message : m.error_file_operation_failed();
    }
  }

  let deleteTargets = $state<DeleteTarget[]>([]);
  let deleteOpen = $state(false);

  function askDelete(entries: DirectoryEntry[]) {
    deleteTargets = entries.map((e) => ({ path: e.path, name: e.name, is_dir: e.is_dir }));
    deleteOpen = true;
  }

  function trashed(items: TrashItem[]) {
    fb.remove(items.map((i) => i.original_path));
    for (const i of items) selected.delete(i.original_path);
    if (preview && items.some((i) => i.original_path === preview)) preview = null;
    deepResults = deepResults.filter((e) => !items.some((i) => i.original_path === e.path));
    fb.loadPlaces();
    showToast({
      message: items.length === 1 ? m.filebrowser_trashed_one({ name: items[0].name }) : m.filebrowser_trashed_many({ count: items.length }),
      variant: 'success',
      duration: 8000,
      action: {
        label: m.filebrowser_undo(),
        onclick: async () => {
          try {
            const result = await api.files.restore(items.map((i) => ({ root: i.root, id: i.id })), true);
            if (result.failed.length > 0) showToast({ message: m.filebrowser_restore_failed(), variant: 'error' });
          } catch (e) {
            showToast({ message: e instanceof Error ? e.message : m.filebrowser_restore_failed(), variant: 'error' });
          }
          fb.refresh();
          fb.loadPlaces();
        },
      },
    });
  }

  function deleted(paths: string[]) {
    fb.remove(paths);
    for (const p of paths) selected.delete(p);
    if (preview && paths.includes(preview)) preview = null;
    deepResults = deepResults.filter((e) => !paths.includes(e.path));
    fb.loadPlaces();
    showToast({ message: m.filebrowser_deleted_forever({ count: paths.length }), variant: 'success' });
  }

  function menuFor(entry: DirectoryEntry): MenuItem[] {
    const many = targetsFor(entry);
    if (many.length > 1) {
      // Right-click inside a selection acts on the whole selection
      return [
        { label: m.filebrowser_download_selected({ count: many.length }), icon: 'download', onclick: () => downloadMany(many) },
        { label: m.filebrowser_copy_to(), icon: 'copy', onclick: () => askTransfer('copy', many) },
        { label: m.filebrowser_move_to(), icon: 'arrows', onclick: () => askTransfer('move', many) },
        'separator',
        { label: m.filebrowser_move_to_trash(), icon: 'trash', danger: true, onclick: () => askDelete(many) },
      ];
    }
    const items: MenuItem[] = [];
    if (entry.is_dir) items.push({ label: m.filebrowser_open(), icon: 'folder-open', onclick: () => go(entry.path) });
    else items.push({ label: m.filebrowser_preview(), icon: 'eye', onclick: () => showPreview(entry) });
    items.push({ label: m.filebrowser_download(), icon: 'download', onclick: () => downloadMany([entry]) });
    if (deepActive) items.push({ label: m.filebrowser_show_in_folder(), icon: 'folder', onclick: () => go(parentOf(entry.path)) });
    items.push('separator');
    items.push({ label: m.filebrowser_rename(), icon: 'edit', onclick: () => askRename(entry) });
    items.push({ label: m.filebrowser_copy_to(), icon: 'copy', onclick: () => askTransfer('copy', [entry]) });
    items.push({ label: m.filebrowser_move_to(), icon: 'arrows', onclick: () => askTransfer('move', [entry]) });
    items.push({ label: m.filebrowser_copy_path(), icon: 'link', onclick: () => copyPath(entry.path) });
    items.push('separator');
    items.push({ label: m.filebrowser_move_to_trash(), icon: 'trash', danger: true, onclick: () => askDelete([entry]) });
    return items;
  }

  // ── Drives ──

  let busyDrive = $state<string | null>(null);
  let placesSheet = $state(false);

  async function mountDrive(drive: FileDrive) {
    busyDrive = drive.uuid;
    try {
      const mountPoint = await fb.mount(drive);
      placesSheet = false;
      if (mountPoint) go(mountPoint);
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_mount_drive(), variant: 'error' });
    } finally {
      busyDrive = null;
    }
  }

  async function unmountDrive(drive: FileDrive) {
    busyDrive = drive.uuid;
    const wasOpen = !!drive.mountpoint && fb.root === drive.mountpoint;
    try {
      await fb.unmount(drive);
      showToast({ message: m.filebrowser_ejected({ name: getDriveFullName(drive) }), variant: 'success' });
      if (wasOpen) replace('/files');
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_unmount_drive(), variant: 'error' });
    } finally {
      busyDrive = null;
    }
  }

  function openPlace(path: string) {
    placesSheet = false;
    go(path);
  }

  // ── New folder ──

  let newFolderOpen = $state(false);
  let newFolderName = $state('');
  let newFolderError = $state('');

  function askNewFolder() {
    newFolderName = '';
    newFolderError = '';
    newFolderOpen = true;
  }

  async function createFolder(e: Event) {
    e.preventDefault();
    const name = newFolderName.trim();
    if (!name) {
      newFolderError = m.error_field_required({ field: m.filebrowser_folder_name() });
      return;
    }
    if (name.includes('/') || name === '.' || name === '..' || name.includes('..')) {
      newFolderError = m.filebrowser_name_invalid();
      return;
    }
    try {
      await api.system.mkdir(`${fb.path.replace(/\/+$/, '')}/${name}`);
      newFolderOpen = false;
      await fb.refresh();
    } catch (err) {
      newFolderError = err instanceof Error ? err.message : m.error_create_folder();
    }
  }

  // ── Page ──

  let pathEditing = $state(false);
  let narrow = $state(false);
  let wide = $state(false);
  /** The preview sits next to the list on wide screens */
  const docked = $derived(wide && !!previewEntry);
  const sidebarNarrow = $derived(fb.prefs.sidebarCollapsed || docked);

  onMount(() => {
    fb.loadPlaces();
    const media = window.matchMedia('(max-width: 639px)');
    const wideMedia = window.matchMedia('(min-width: 1280px)');
    narrow = media.matches;
    wide = wideMedia.matches;
    const onMedia = () => {
      narrow = media.matches;
      wide = wideMedia.matches;
    };
    media.addEventListener('change', onMedia);
    wideMedia.addEventListener('change', onMedia);

    // Uploads into the open folder show up (at most once a second)
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;
    const offUploaded = uploader.onFinished((item) => {
      if (item.state !== 'done' || item.dir !== fb.path || refreshTimer) return;
      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        fb.refresh();
        fb.loadPlaces();
      }, 1000);
    });
    // Leaving would stop the uploads (they resume when the files are picked again)
    const beforeUnload = (e: BeforeUnloadEvent) => {
      if (uploader.open.length > 0) e.preventDefault();
    };
    window.addEventListener('beforeunload', beforeUnload);

    // Copies and moves: refresh the folder when one ends that touched it
    const disconnectTasks = fileTasks.connect();
    const offFinished = fileTasks.onFinished((task) => {
      const touched = task.target === fb.path || task.sources.some((s) => parentOf(s) === fb.path);
      if (touched) fb.refresh();
      fb.loadPlaces();
      if (task.state === 'done' && task.errors.length === 0) {
        showToast({ message: task.kind === 'copy' ? m.filebrowser_copy_done() : m.filebrowser_move_done(), variant: 'success' });
      } else if (task.state === 'done' || task.state === 'failed') {
        showToast({
          message: task.failure ?? m.filebrowser_task_done_errors({ count: task.errors.length }),
          variant: task.state === 'failed' ? 'error' : 'warning',
        });
      }
    });
    function onKey(e: KeyboardEvent) {
      const t = e.target as HTMLElement;
      const typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t.isContentEditable;
      if (!fb.path || trashView) return;
      if ((e.key === '/' && !typing) || ((e.ctrlKey || e.metaKey) && e.key === 'f')) {
        e.preventDefault();
        searchOpen = true;
      } else if ((e.ctrlKey || e.metaKey) && e.key === 'l') {
        e.preventDefault();
        pathEditing = true;
      } else if (e.key === 'Escape' && searchOpen && !typing) {
        closeSearch();
      }
    }
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      media.removeEventListener('change', onMedia);
      wideMedia.removeEventListener('change', onMedia);
      offFinished();
      disconnectTasks();
      offUploaded();
      if (refreshTimer) clearTimeout(refreshTimer);
      window.removeEventListener('beforeunload', beforeUnload);
    };
  });

  const toolbarMenu = $derived<MenuItem[]>([
    { label: m.filebrowser_new_folder(), icon: 'plus', onclick: askNewFolder, disabled: !fb.path },
    { label: m.filebrowser_upload_files(), icon: 'upload', onclick: () => fileInput?.click(), disabled: !fb.path },
    { label: m.filebrowser_upload_folder(), icon: 'upload', onclick: () => folderInput?.click(), disabled: !fb.path },
    {
      label: fb.prefs.viewMode === 'list' ? m.filebrowser_view_grid() : m.filebrowser_view_list(),
      icon: fb.prefs.viewMode === 'list' ? 'grid' : 'list',
      onclick: () => fb.setViewMode(fb.prefs.viewMode === 'list' ? 'grid' : 'list'),
    },
    'separator',
    { label: m.filebrowser_refresh(), icon: 'refresh', onclick: () => fb.refresh(), disabled: !fb.path },
    { label: m.filebrowser_copy_folder_path(), icon: 'copy', onclick: () => copyPath(fb.path), disabled: !fb.path },
  ]);

  const rootLabel = $derived.by(() => {
    if (trashView) return m.filebrowser_trash();
    if (!fb.root) return m.filebrowser_places();
    const drive = fb.places?.drives.find((d) => d.mountpoint === fb.root);
    return drive ? getDriveFullName(drive) : (fb.root.split('/').filter(Boolean).pop() ?? fb.root);
  });

  const status = $derived.by(() => {
    const list = searchQuery.trim() ? shown : fb.entries;
    const folders = list.filter((e) => e.is_dir).length;
    const files = list.length - folders;
    const bytes = list.reduce((sum, e) => sum + (e.size ?? 0), 0);
    return { folders, files, bytes };
  });

  const emptyText = $derived(
    searchQuery.trim()
      ? deepActive && deepLoading
        ? m.filebrowser_searching()
        : m.filebrowser_search_no_results({ query: searchQuery.trim() })
      : m.filebrowser_empty(),
  );
</script>

<PageHeader title={m.nav_files()} />

<div
  class="lg:grid gap-6 items-start"
  style="grid-template-columns: {sidebarNarrow ? '3.25rem' : '15rem'} minmax(0, 1fr){docked ? ' 24rem' : ''}"
>
  <aside class="hidden lg:flex flex-col sticky top-8 max-h-[calc(100vh-4rem)] overflow-y-auto -ml-2 pr-1">
    <PlacesPanel
      places={fb.places}
      loading={fb.placesLoading}
      activeRoot={fb.root}
      trashActive={trashView}
      collapsed={sidebarNarrow}
      {busyDrive}
      onOpen={openPlace}
      onTrash={openTrash}
      onMount={mountDrive}
      onUnmount={unmountDrive}
      onRefresh={() => fb.loadPlaces()}
    />
    <button
      type="button"
      class="btn btn-ghost btn-icon btn-sm mt-4 self-start ml-1"
      onclick={() => fb.toggleSidebar()}
      aria-label={fb.prefs.sidebarCollapsed ? m.filebrowser_sidebar_expand() : m.filebrowser_sidebar_collapse()}
      title={fb.prefs.sidebarCollapsed ? m.filebrowser_sidebar_expand() : m.filebrowser_sidebar_collapse()}
    >
      <Icon name={fb.prefs.sidebarCollapsed ? 'chevron-right' : 'chevron-left'} class="w-4 h-4" />
    </button>
  </aside>

  <section
    class="card min-w-0 relative"
    aria-label={m.nav_files()}
    ondragenter={(e) => {
      if (hasFiles(e) && fb.path && !trashView) dragDepth++;
    }}
    ondragleave={() => (dragDepth = Math.max(0, dragDepth - 1))}
    ondragover={(e) => {
      if (hasFiles(e) && fb.path && !trashView) e.preventDefault();
    }}
    ondrop={onDrop}
  >
    {#if dragDepth > 0}
      <div
        class="absolute inset-0 z-20 rounded-xl border-2 border-dashed border-primary-500 bg-primary-50/90 dark:bg-primary-900/60 flex flex-col items-center justify-center gap-2 pointer-events-none"
      >
        <Icon name="upload" class="w-8 h-8 text-primary-600 dark:text-primary-300" />
        <p class="text-sm font-medium text-primary-800 dark:text-primary-200">
          {m.filebrowser_drop_here({ folder: fb.path.split('/').filter(Boolean).pop() ?? fb.path })}
        </p>
      </div>
    {/if}
    {#if selected.size > 0}
      <div
        class="flex items-center gap-1 px-2 sm:px-3 py-2 border-b border-primary-200 dark:border-primary-800 bg-primary-50/80 dark:bg-primary-900/25 min-h-[3.25rem] rounded-t-xl"
        role="toolbar"
        aria-label={m.filebrowser_selection_actions()}
      >
        <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={() => selected.clear()} aria-label={m.filebrowser_clear_selection()} title={m.filebrowser_clear_selection()}>
          <Icon name="x" class="w-4 h-4" />
        </button>
        <span class="flex-1 min-w-0 truncate px-1 text-sm font-medium text-gray-900 dark:text-white tabular-nums">
          {m.filebrowser_selected({ count: selected.size })}<span class="hidden sm:inline font-normal text-gray-500 dark:text-gray-400">&nbsp;· {formatBytes(selectedBytes)}</span>
        </span>
        <button type="button" class="btn btn-ghost btn-sm" onclick={() => downloadMany(selectedEntries)} title={m.filebrowser_download()}>
          <Icon name="download" class="w-4 h-4" /><span class="hidden md:inline">{m.filebrowser_download()}</span>
        </button>
        <button type="button" class="btn btn-ghost btn-sm" onclick={() => askTransfer('copy', selectedEntries)} title={m.filebrowser_copy_to()}>
          <Icon name="copy" class="w-4 h-4" /><span class="hidden md:inline">{m.filebrowser_copy_to()}</span>
        </button>
        <button type="button" class="btn btn-ghost btn-sm" onclick={() => askTransfer('move', selectedEntries)} title={m.filebrowser_move_to()}>
          <Icon name="arrows" class="w-4 h-4" /><span class="hidden md:inline">{m.filebrowser_move_to()}</span>
        </button>
        {#if selectedEntries.length === 1}
          <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={() => askRename(selectedEntries[0])} aria-label={m.filebrowser_rename()} title={m.filebrowser_rename()}>
            <Icon name="edit" class="w-4 h-4" />
          </button>
        {/if}
        <button type="button" class="btn btn-danger-ghost btn-sm" onclick={() => askDelete(selectedEntries)} title={m.filebrowser_move_to_trash()}>
          <Icon name="trash" class="w-4 h-4" /><span class="hidden md:inline">{m.filebrowser_trash_short()}</span>
        </button>
      </div>
    {:else}
    <div class="flex items-center gap-1 px-2 sm:px-3 py-2 border-b border-gray-200 dark:border-gray-700 min-h-[3.25rem]">
      <button
        type="button"
        class="lg:hidden btn btn-ghost btn-sm gap-1.5 max-w-[40%] shrink-0"
        onclick={() => (placesSheet = true)}
        aria-haspopup="dialog"
        aria-label={m.filebrowser_choose_place()}
      >
        <Icon name={trashView ? 'trash' : 'drive'} class="w-4 h-4 shrink-0" />
        <span class="truncate">{rootLabel}</span>
        <Icon name="chevron-down" class="w-3.5 h-3.5 shrink-0" />
      </button>

      {#if trashView}
        <h2 class="flex-1 px-1.5 text-sm font-medium text-gray-900 dark:text-white hidden lg:block">{m.filebrowser_trash()}</h2>
        <div class="flex-1 lg:hidden"></div>
      {:else if fb.path}
        <button
          type="button"
          class="btn btn-ghost btn-icon btn-sm shrink-0"
          onclick={goUp}
          disabled={atRoot}
          aria-label={m.filebrowser_go_up()}
          title={m.filebrowser_go_up()}
        >
          <Icon name="arrow-up" class="w-4 h-4" />
        </button>
        <PathBar path={fb.path} root={fb.root} compact={narrow} bind:editing={pathEditing} onNavigate={go} />
        <button
          type="button"
          class="btn btn-ghost btn-icon btn-sm shrink-0 {searchOpen ? 'bg-gray-100 dark:bg-gray-800' : ''}"
          onclick={() => (searchOpen ? closeSearch() : (searchOpen = true))}
          aria-pressed={searchOpen}
          aria-label={m.filebrowser_search_toggle()}
          title={m.filebrowser_search_toggle()}
        >
          <Icon name="search" class="w-4 h-4" />
        </button>
        <div class="hidden sm:flex items-center rounded-lg bg-gray-100 dark:bg-gray-800 p-0.5 shrink-0" role="group" aria-label={m.filebrowser_view()}>
          {#each [{ mode: 'list', icon: 'list', label: m.filebrowser_view_list() }, { mode: 'grid', icon: 'grid', label: m.filebrowser_view_grid() }] as const as v (v.mode)}
            <button
              type="button"
              class="p-1.5 rounded-md {fb.prefs.viewMode === v.mode ? 'bg-white dark:bg-gray-700 shadow-sm text-gray-900 dark:text-white' : 'text-gray-500 hover:text-gray-800 dark:hover:text-gray-200'}"
              aria-pressed={fb.prefs.viewMode === v.mode}
              aria-label={v.label}
              title={v.label}
              onclick={() => fb.setViewMode(v.mode)}
            >
              <Icon name={v.icon} class="w-4 h-4" />
            </button>
          {/each}
        </div>
        <FileTasks />
        <Menu label={m.filebrowser_more_actions()} items={toolbarMenu} triggerClass="btn btn-ghost btn-icon btn-sm shrink-0" />
        <span class="hidden sm:inline-flex shrink-0 ml-1">
          <Menu label={m.filebrowser_upload()} items={uploadItems} triggerClass="btn btn-secondary btn-sm">
            {#snippet trigger()}<Icon name="upload" class="w-4 h-4" />{m.filebrowser_upload()}{/snippet}
          </Menu>
        </span>
        <button type="button" class="hidden md:inline-flex btn btn-secondary btn-sm shrink-0" onclick={askNewFolder}>
          <Icon name="plus" class="w-4 h-4" />{m.filebrowser_new_folder()}
        </button>
      {:else}
        <p class="flex-1 px-1.5 text-sm text-gray-500 dark:text-gray-400 hidden lg:block">{m.filebrowser_select_location_desc()}</p>
        <FileTasks />
      {/if}
    </div>
    {/if}

    {#if trashView}
      <TrashView onChanged={() => fb.loadPlaces()} onOpenFolder={go} />
    {:else if fb.path}
      {#if searchOpen}
        <FileSearchBar
          bind:query={searchQuery}
          matchCount={shown.length}
          totalCount={fb.entries.length}
          onClose={closeSearch}
          {searchMode}
          onSearchModeChange={setSearchMode}
          onSearch={deepSearch}
          deepSearchLoading={deepLoading}
          deepSearchTruncated={deepTruncated}
          deepSearchTimedOut={deepTimedOut}
          deepSearchCount={deepResults.length}
        />
      {/if}
      <FileList
        entries={shown}
        view={fb.prefs.viewMode}
        sortBy={fb.prefs.sortBy}
        sortOrder={fb.prefs.sortOrder}
        loading={fb.loading || (deepActive && deepLoading && shown.length === 0)}
        error={deepActive ? null : fb.error}
        sizesLoading={fb.sizesLoading}
        {searchQuery}
        searchBase={deepActive ? deepBase : ''}
        {emptyText}
        onSort={(field) => fb.sortBy(field)}
        onRetry={() => fb.refresh()}
        onOpen={open}
        {menuFor}
        onDelete={(entry) => askDelete(targetsFor(entry))}
        onUp={goUp}
        onRename={(entry) => !deepActive && askRename(entry)}
        {selected}
        thumbs
        onActiveChange={(entry) => {
          if (preview && !entry.is_dir) preview = entry.path;
        }}
      />
      {#if !fb.error && fb.entries.length > 0}
        <div
          class="flex items-center justify-between gap-3 px-4 py-2 border-t border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400 tabular-nums"
          role="status"
        >
          <span>
            {status.folders === 1 ? m.filebrowser_folders_one() : m.filebrowser_folders_many({ count: status.folders })}
            · {status.files === 1 ? m.filebrowser_files_one() : m.filebrowser_files_many({ count: status.files })}
            · {formatBytes(status.bytes)}
          </span>
          {#if fb.sizesLoading}<span>{m.filebrowser_sizes_loading()}</span>{/if}
        </div>
      {/if}
    {:else}
      <div class="lg:hidden p-3">
        <PlacesPanel
          places={fb.places}
          loading={fb.placesLoading}
          activeRoot={null}
          trashActive={false}
          {busyDrive}
          onOpen={openPlace}
          onTrash={openTrash}
          onMount={mountDrive}
          onUnmount={unmountDrive}
          onRefresh={() => fb.loadPlaces()}
        />
      </div>
      <div class="hidden lg:block py-20 px-6 text-center text-sm text-gray-500 dark:text-gray-400">
        <Icon name="folder-open" class="w-9 h-9 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
        <p class="font-medium text-gray-700 dark:text-gray-300">{m.filebrowser_select_location()}</p>
        <p class="mt-1">{m.filebrowser_select_location_desc()}</p>
      </div>
    {/if}
  </section>

  {#if docked && previewEntry}
    <aside class="card sticky top-8 max-h-[calc(100vh-4rem)] flex flex-col overflow-hidden">
      {@render previewPanel(previewEntry)}
    </aside>
  {/if}
</div>

{#snippet previewPanel(entry: DirectoryEntry)}
  <FilePreview
    {entry}
    hasPrev={previewIndex > 0}
    hasNext={previewIndex >= 0 && previewIndex < previewFiles.length - 1}
    playing={playingPath === entry.path}
    onPrev={() => stepPreview(-1)}
    onNext={() => stepPreview(1)}
    onClose={() => (preview = null)}
    onDownload={() => download(entry)}
    onRename={() => askRename(entry)}
    onTrash={() => askDelete([entry])}
    onCopyPath={() => copyPath(entry.path)}
    onPlay={() => play(entry)}
  />
{/snippet}

{#if !docked}
  <Dialog
    open={!!previewEntry}
    title={previewEntry?.name ?? ''}
    size="xl"
    bodyClass="p-0 h-[75dvh] flex flex-col"
    onclose={() => (preview = null)}
  >
    {#if previewEntry}{@render previewPanel(previewEntry)}{/if}
  </Dialog>
{/if}

{#if playerQueue.length > 0}
  <MusicPlayer queue={playerQueue} bind:index={playerIndex} onClose={() => (playerQueue = [])} />
{/if}

<ConflictDialog />
<UploadConflictDialog />

<input
  bind:this={fileInput}
  type="file"
  multiple
  class="hidden"
  onchange={(e) => {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files) upload([...input.files].map((file) => ({ file, relDir: '' })));
    input.value = '';
  }}
/>
<input
  bind:this={folderInput}
  type="file"
  webkitdirectory
  class="hidden"
  onchange={(e) => {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files) upload(filesFromFolderInput(input.files));
    input.value = '';
  }}
/>

<BrowseModal
  bind:open={pickerOpen}
  rootPath={fb.root ?? fb.path}
  startPath={fb.path}
  allowedPaths={fb.roots}
  title={pickerKind === 'copy' ? m.filebrowser_copy_to_title({ count: pickerSources.length }) : m.filebrowser_move_to_title({ count: pickerSources.length })}
  confirmLabel={pickerKind === 'copy' ? m.filebrowser_copy_here() : m.filebrowser_move_here()}
  onSelect={transfer}
  onClose={() => (pickerOpen = false)}
/>

<Dialog bind:open={renameOpen} title={m.filebrowser_rename()} description={renameEntry?.name} size="sm">
  <form id="rename-form" onsubmit={doRename}>
    <label for="rename-name" class="label">{m.filebrowser_new_name()}</label>
    <input bind:this={renameInput} type="text" id="rename-name" bind:value={renameValue} class="input" autocomplete="off" spellcheck="false" />
    {#if renameError}<p class="mt-1.5 text-sm text-red-600 dark:text-red-400" role="alert">{renameError}</p>{/if}
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-secondary" onclick={() => (renameOpen = false)}>{m.common_cancel()}</button>
    <button type="submit" form="rename-form" class="btn btn-primary">{m.filebrowser_rename()}</button>
  {/snippet}
</Dialog>

<Dialog bind:open={placesSheet} title={m.filebrowser_choose_place()} size="sm">
  <PlacesPanel
    places={fb.places}
    loading={fb.placesLoading}
    activeRoot={fb.root}
    trashActive={trashView}
    {busyDrive}
    onOpen={openPlace}
    onTrash={openTrash}
    onMount={mountDrive}
    onUnmount={unmountDrive}
    onRefresh={() => fb.loadPlaces()}
  />
</Dialog>

<Dialog bind:open={newFolderOpen} title={m.filebrowser_new_folder()} description={fb.path} size="sm">
  <form id="new-folder-form" onsubmit={createFolder}>
    <label for="folder-name" class="label">{m.filebrowser_folder_name()}</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input type="text" id="folder-name" bind:value={newFolderName} class="input" autocomplete="off" autofocus />
    {#if newFolderError}<p class="mt-1.5 text-sm text-red-600 dark:text-red-400" role="alert">{newFolderError}</p>{/if}
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-secondary" onclick={() => (newFolderOpen = false)}>{m.common_cancel()}</button>
    <button type="submit" form="new-folder-form" class="btn btn-primary">{m.common_create()}</button>
  {/snippet}
</Dialog>

<TrashDialog bind:open={deleteOpen} targets={deleteTargets} onTrashed={trashed} onDeleted={deleted} />

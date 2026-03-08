import { writable } from 'svelte/store';
import { api } from '../api';
import type { DirectoryEntry, UsbDrive } from '../types';
import { showToast } from '../../components/ui/Toast.svelte';
import * as m from '$lib/paraglide/messages.js';

function translateDownloadError(code: string, _params?: Record<string, string | number>): string {
  switch (code) {
    case 'FILE_TOO_LARGE':             return String(m.error_file_too_large());
    case 'FILE_NOT_FOUND':             return String(m.error_file_not_found());
    case 'NOT_A_FILE':                 return String(m.error_not_a_file());
    case 'PATH_NOT_ALLOWED':
    case 'PATH_TRAVERSAL_NOT_ALLOWED': return String(m.error_path_not_allowed());
    case 'DOWNLOAD_FAILED':
    default:                           return String(m.error_download_failed());
  }
}

export type SortField = 'name' | 'size' | 'modified';
export type SortOrder = 'asc' | 'desc';
export type ViewMode = 'list' | 'grid';

interface FileBrowserState {
  currentPath: string;
  pathHistory: string[];
  entries: DirectoryEntry[];
  loading: boolean;
  downloading: string | null;
  deleting: string | null;
  deleteVerifiedUntil: number | null;
  drives: UsbDrive[];
  loadingDrives: boolean;
  allowedPaths: string[];
  viewMode: ViewMode;
  sortBy: SortField;
  sortOrder: SortOrder;
  error: string | null;
}

const STORAGE_KEYS = {
  viewMode: 'dynamight-filebrowser-view',
  sortBy: 'dynamight-filebrowser-sort',
  sortOrder: 'dynamight-filebrowser-order',
};

function loadPreference<T>(key: string, defaultValue: T): T {
  if (typeof window === 'undefined') return defaultValue;
  try {
    const stored = localStorage.getItem(key);
    return stored ? (JSON.parse(stored) as T) : defaultValue;
  } catch {
    return defaultValue;
  }
}

function savePreference(key: string, value: unknown): void {
  if (typeof window === 'undefined') return;
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Ignore storage errors
  }
}

function sortEntries(entries: DirectoryEntry[], sortBy: SortField, sortOrder: SortOrder): DirectoryEntry[] {
  return [...entries].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;

    let comparison = 0;
    switch (sortBy) {
      case 'name':     comparison = a.name.toLowerCase().localeCompare(b.name.toLowerCase()); break;
      case 'size':     comparison = (a.size ?? 0) - (b.size ?? 0); break;
      case 'modified': comparison = (a.modified ?? 0) - (b.modified ?? 0); break;
    }
    return sortOrder === 'asc' ? comparison : -comparison;
  });
}

function createFileBrowserStore() {
  const initialState: FileBrowserState = {
    currentPath: '',
    pathHistory: [],
    entries: [],
    loading: false,
    downloading: null,
    deleting: null,
    deleteVerifiedUntil: null,
    drives: [],
    loadingDrives: false,
    allowedPaths: [],
    viewMode: loadPreference(STORAGE_KEYS.viewMode, 'list'),
    sortBy: loadPreference(STORAGE_KEYS.sortBy, 'name'),
    sortOrder: loadPreference(STORAGE_KEYS.sortOrder, 'asc'),
    error: null,
  };

  const { subscribe, set, update } = writable<FileBrowserState>(initialState);

  return {
    subscribe,

    async browsePath(path: string): Promise<boolean> {
      let previousPath = '';
      update((s) => {
        previousPath = s.currentPath;
        return { ...s, loading: true, error: null, currentPath: path };
      });
      try {
        const result = await api.system.browse(path);
        update((s) => {
          const sortedEntries = sortEntries(result.entries, s.sortBy, s.sortOrder);
          const newHistory = previousPath && previousPath !== result.path
            ? [...s.pathHistory, previousPath]
            : s.pathHistory;
          return { ...s, currentPath: result.path, entries: sortedEntries, pathHistory: newHistory, loading: false, error: null };
        });
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(m.error_browse_path());
        update((s) => ({ ...s, loading: false, error: message, currentPath: previousPath }));
        return false;
      }
    },

    async goUp(): Promise<boolean> {
      let currentPath = '';
      update((s) => { currentPath = s.currentPath; return s; });
      if (!currentPath || currentPath === '/') return false;
      const parentPath = currentPath.split('/').slice(0, -1).join('/') || '/';
      return this.browsePath(parentPath);
    },

    async goBack(): Promise<boolean> {
      let previousPath = '';
      update((s) => {
        if (s.pathHistory.length > 0) {
          previousPath = s.pathHistory[s.pathHistory.length - 1];
          return { ...s, pathHistory: s.pathHistory.slice(0, -1) };
        }
        return s;
      });
      if (previousPath) return this.browsePath(previousPath);
      return false;
    },

    async loadDrives(): Promise<void> {
      update((s) => ({ ...s, loadingDrives: true }));
      try {
        const drives = await api.system.drives();
        update((s) => ({ ...s, drives, loadingDrives: false }));
      } catch {
        update((s) => ({ ...s, loadingDrives: false }));
      }
    },

    async loadAllowedPaths(): Promise<void> {
      try {
        const result = await api.system.allowedPaths();
        update((s) => ({ ...s, allowedPaths: result.paths }));
      } catch {
        // Silent failure
      }
    },

    async mountDrive(uuid: string, mountPoint: string): Promise<boolean> {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await api.system.mount(uuid, mountPoint);
        await this.loadDrives();
        update((s) => ({ ...s, loading: false }));
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(m.error_mount_drive());
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async unmountDrive(mountPoint: string): Promise<boolean> {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await api.system.unmount(mountPoint);
        await this.loadDrives();
        update((s) => ({ ...s, loading: false }));
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(m.error_unmount_drive());
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async generateMountPoint(uuid: string, label?: string): Promise<string | null> {
      try {
        const result = await api.system.generateMountPoint(uuid, label);
        return result.mount_point;
      } catch {
        return null;
      }
    },

    async downloadFile(path: string): Promise<void> {
      update((s) => ({ ...s, downloading: path }));
      try {
        // Use fetch to properly handle URL encoding and authentication
        const url = api.system.downloadUrl(path);
        const response = await fetch(url, { credentials: 'include' });

        if (!response.ok) {
          const text = await response.text();
          let errorMessage = String(m.error_download_failed());
          try {
            const errorData = JSON.parse(text);
            if (errorData.code) {
              errorMessage = translateDownloadError(errorData.code, errorData.params);
            } else if (errorData.error) {
              errorMessage = errorData.error;
            }
          } catch {
            // Use default error message
          }
          update((s) => ({ ...s, downloading: null }));
          showToast({ message: errorMessage, variant: 'error' });
          return;
        }

        // Get filename from Content-Disposition header or path
        const contentDisposition = response.headers.get('Content-Disposition');
        let filename = path.split('/').pop() || 'download';
        if (contentDisposition) {
          const match = contentDisposition.match(/filename="?([^";\n]+)"?/);
          if (match) filename = match[1];
        }

        // Create blob and trigger download
        const blob = await response.blob();
        const blobUrl = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = blobUrl;
        link.download = filename;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        URL.revokeObjectURL(blobUrl);

        update((s) => ({ ...s, downloading: null }));
      } catch (e) {
        update((s) => ({ ...s, downloading: null }));
        showToast({
          message: e instanceof Error ? e.message : String(m.error_download_failed()),
          variant: 'error',
        });
      }
    },

    async createFolder(path: string): Promise<boolean> {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await api.system.mkdir(path);
        let currentPath = '';
        update((s) => { currentPath = s.currentPath; return s; });
        if (currentPath) await this.browsePath(currentPath);
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(m.error_create_folder());
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async checkDeleteStatus(): Promise<void> {
      try {
        const result = await api.system.deleteStatus();
        update((s) => ({
          ...s,
          deleteVerifiedUntil: result.verified && result.expires_at ? result.expires_at : null,
        }));
      } catch {
        update((s) => ({ ...s, deleteVerifiedUntil: null }));
      }
    },

    isDeleteVerified(): boolean {
      let verified = false;
      update((s) => {
        const now = Math.floor(Date.now() / 1000);
        verified = s.deleteVerifiedUntil !== null && s.deleteVerifiedUntil > now;
        return s;
      });
      return verified;
    },

    async verifyDeleteAccess(password: string, totpCode?: string): Promise<boolean> {
      try {
        const result = await api.system.verifyDeleteAccess(password, totpCode);
        if (result.verified) {
          update((s) => ({ ...s, deleteVerifiedUntil: result.expires_at }));
          return true;
        }
        return false;
      } catch (e) {
        const message = e instanceof Error ? e.message : String(m.error_delete_verification_failed());
        showToast({ message, variant: 'error' });
        return false;
      }
    },

    async deleteEntry(path: string): Promise<'success' | 'verification_required' | 'error'> {
      const now = Math.floor(Date.now() / 1000);
      let verifiedUntil: number | null = null;
      update((s) => { verifiedUntil = s.deleteVerifiedUntil; return s; });

      if (!verifiedUntil || verifiedUntil <= now) return 'verification_required';

      update((s) => ({ ...s, deleting: path }));
      try {
        const result = await api.system.deleteFile(path);
        await this.checkDeleteStatus();

        let currentPath = '';
        update((s) => { currentPath = s.currentPath; return { ...s, deleting: null }; });
        if (currentPath) await this.browsePath(currentPath);

        const successMessage = result.is_dir
          ? String(m.filebrowser_delete_success_folder())
          : String(m.filebrowser_delete_success());
        showToast({ message: successMessage, variant: 'success' });
        return 'success';
      } catch (e) {
        update((s) => ({ ...s, deleting: null }));
        if (e instanceof Error && e.message.includes('verification')) {
          update((s) => ({ ...s, deleteVerifiedUntil: null }));
          return 'verification_required';
        }
        showToast({
          message: e instanceof Error ? e.message : String(m.error_delete_failed()),
          variant: 'error',
        });
        return 'error';
      }
    },

    setViewMode(mode: ViewMode): void {
      savePreference(STORAGE_KEYS.viewMode, mode);
      update((s) => ({ ...s, viewMode: mode }));
    },

    setSortBy(field: SortField): void {
      savePreference(STORAGE_KEYS.sortBy, field);
      update((s) => ({ ...s, sortBy: field, entries: sortEntries(s.entries, field, s.sortOrder) }));
    },

    toggleSortOrder(): void {
      update((s) => {
        const newOrder = s.sortOrder === 'asc' ? 'desc' : 'asc';
        savePreference(STORAGE_KEYS.sortOrder, newOrder);
        return { ...s, sortOrder: newOrder, entries: sortEntries(s.entries, s.sortBy, newOrder) };
      });
    },

    clearError(): void {
      update((s) => ({ ...s, error: null }));
    },

    reset(): void {
      set({
        ...initialState,
        deleting: null,
        deleteVerifiedUntil: null,
        viewMode: loadPreference(STORAGE_KEYS.viewMode, 'list'),
        sortBy: loadPreference(STORAGE_KEYS.sortBy, 'name'),
        sortOrder: loadPreference(STORAGE_KEYS.sortOrder, 'asc'),
      });
    },
  };
}

export const fileBrowserStore = createFileBrowserStore();

import { writable } from 'svelte/store';
import { api } from '../api';
import type { DirectoryEntry, UsbDrive } from '../types';
import { showToast } from '../../components/ui/Toast.svelte';
import * as m from '$lib/paraglide/messages.js';

// Helper to translate download error codes to user-friendly messages
function translateDownloadError(code: string, _params?: Record<string, string | number>): string {
  switch (code) {
    case 'FILE_TOO_LARGE':
      return String(m.error_file_too_large());
    case 'FILE_NOT_FOUND':
      return String(m.error_file_not_found());
    case 'NOT_A_FILE':
      return String(m.error_not_a_file());
    case 'PATH_NOT_ALLOWED':
      return String(m.error_path_not_allowed());
    case 'PATH_TRAVERSAL_NOT_ALLOWED':
      return String(m.error_path_not_allowed());
    case 'DOWNLOAD_FAILED':
      return String(m.error_download_failed());
    default:
      return String(m.error_download_failed());
  }
}

export type SortField = 'name' | 'size' | 'modified';
export type SortOrder = 'asc' | 'desc';
export type ViewMode = 'list' | 'grid';

interface FileBrowserState {
  // Navigation
  currentPath: string;
  pathHistory: string[];
  entries: DirectoryEntry[];

  // Loading states
  loading: boolean;
  downloading: string | null;

  // USB Drives
  drives: UsbDrive[];
  loadingDrives: boolean;

  // Allowed paths
  allowedPaths: string[];

  // View preferences
  viewMode: ViewMode;
  sortBy: SortField;
  sortOrder: SortOrder;

  // Errors
  error: string | null;
}

// Local storage keys for preferences
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
  const sorted = [...entries].sort((a, b) => {
    // Directories always come first
    if (a.is_dir !== b.is_dir) {
      return a.is_dir ? -1 : 1;
    }

    let comparison = 0;
    switch (sortBy) {
      case 'name':
        comparison = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
        break;
      case 'size':
        comparison = (a.size ?? 0) - (b.size ?? 0);
        break;
      case 'modified':
        comparison = (a.modified ?? 0) - (b.modified ?? 0);
        break;
    }

    return sortOrder === 'asc' ? comparison : -comparison;
  });

  return sorted;
}

function createFileBrowserStore() {
  const initialState: FileBrowserState = {
    currentPath: '',
    pathHistory: [],
    entries: [],
    loading: false,
    downloading: null,
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

    // Navigation
    async browsePath(path: string): Promise<boolean> {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const result = await api.system.browse(path);

        update((s) => {
          const sortedEntries = sortEntries(result.entries, s.sortBy, s.sortOrder);
          // Add current path to history if navigating to a new path
          const newHistory =
            s.currentPath && s.currentPath !== path
              ? [...s.pathHistory, s.currentPath]
              : s.pathHistory;

          return {
            ...s,
            currentPath: result.path,
            entries: sortedEntries,
            pathHistory: newHistory,
            loading: false,
            error: null,
          };
        });
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to browse path';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async goUp(): Promise<boolean> {
      let currentPath = '';
      update((s) => {
        currentPath = s.currentPath;
        return s;
      });

      if (!currentPath || currentPath === '/') return false;

      const parentPath = currentPath.split('/').slice(0, -1).join('/') || '/';
      return this.browsePath(parentPath);
    },

    async goBack(): Promise<boolean> {
      let previousPath = '';
      update((s) => {
        if (s.pathHistory.length > 0) {
          previousPath = s.pathHistory[s.pathHistory.length - 1];
          return {
            ...s,
            pathHistory: s.pathHistory.slice(0, -1),
          };
        }
        return s;
      });

      if (previousPath) {
        return this.browsePath(previousPath);
      }
      return false;
    },

    // Drive management
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
        // Refresh drives to get updated mount status
        await this.loadDrives();
        update((s) => ({ ...s, loading: false }));
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to mount drive';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async unmountDrive(mountPoint: string): Promise<boolean> {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await api.system.unmount(mountPoint);
        // Refresh drives to get updated mount status
        await this.loadDrives();
        update((s) => ({ ...s, loading: false }));
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to unmount drive';
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

    // File operations
    async downloadFile(path: string): Promise<void> {
      update((s) => ({ ...s, downloading: path }));

      try {
        // Use fetch to properly handle URL encoding and authentication
        const url = api.system.downloadUrl(path);
        const response = await fetch(url, {
          credentials: 'include',
        });

        if (!response.ok) {
          const text = await response.text();
          let errorMessage = String(m.error_download_failed());
          try {
            const errorData = JSON.parse(text);
            if (errorData.code) {
              // Translate error codes to user-friendly messages
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
          if (match) {
            filename = match[1];
          }
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
        // Refresh current directory
        let currentPath = '';
        update((s) => {
          currentPath = s.currentPath;
          return s;
        });
        if (currentPath) {
          await this.browsePath(currentPath);
        }
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Failed to create folder';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    // View preferences
    setViewMode(mode: ViewMode): void {
      savePreference(STORAGE_KEYS.viewMode, mode);
      update((s) => ({ ...s, viewMode: mode }));
    },

    setSortBy(field: SortField): void {
      savePreference(STORAGE_KEYS.sortBy, field);
      update((s) => ({
        ...s,
        sortBy: field,
        entries: sortEntries(s.entries, field, s.sortOrder),
      }));
    },

    toggleSortOrder(): void {
      update((s) => {
        const newOrder = s.sortOrder === 'asc' ? 'desc' : 'asc';
        savePreference(STORAGE_KEYS.sortOrder, newOrder);
        return {
          ...s,
          sortOrder: newOrder,
          entries: sortEntries(s.entries, s.sortBy, newOrder),
        };
      });
    },

    // Utilities
    clearError(): void {
      update((s) => ({ ...s, error: null }));
    },

    reset(): void {
      set({
        ...initialState,
        viewMode: loadPreference(STORAGE_KEYS.viewMode, 'list'),
        sortBy: loadPreference(STORAGE_KEYS.sortBy, 'name'),
        sortOrder: loadPreference(STORAGE_KEYS.sortOrder, 'asc'),
      });
    },
  };
}

export const fileBrowserStore = createFileBrowserStore();

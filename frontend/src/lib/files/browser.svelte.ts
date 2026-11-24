import { api } from '$lib/api';
import type { DirectoryEntry, FileDrive, FilePlaces } from '$lib/types';
import { sortEntries, type SortField, type SortOrder } from './fileSort';
import { streamDirSizes } from './dirSizes';
import * as m from '$lib/paraglide/messages.js';

export type ViewMode = 'list' | 'grid';

interface Prefs {
  sortBy: SortField;
  sortOrder: SortOrder;
  viewMode: ViewMode;
  sidebarCollapsed: boolean;
}

const PREFS_KEY = 'dynamight-files';
const DEFAULT_PREFS: Prefs = { sortBy: 'name', sortOrder: 'asc', viewMode: 'list', sidebarCollapsed: false };
/** Settings of the file browser before 2.2 */
const OLD_KEYS = [
  'dynamight-filebrowser-view',
  'dynamight-filebrowser-sort',
  'dynamight-filebrowser-order',
  'dynamight-filebrowser-table-prefs',
];

function loadPrefs(): Prefs {
  if (typeof localStorage === 'undefined') return { ...DEFAULT_PREFS };
  try {
    for (const key of OLD_KEYS) localStorage.removeItem(key);
    const stored = JSON.parse(localStorage.getItem(PREFS_KEY) ?? '{}') as Partial<Prefs>;
    return {
      sortBy: ['name', 'size', 'modified'].includes(stored.sortBy ?? '') ? stored.sortBy! : DEFAULT_PREFS.sortBy,
      sortOrder: stored.sortOrder === 'desc' ? 'desc' : 'asc',
      viewMode: stored.viewMode === 'grid' ? 'grid' : 'list',
      sidebarCollapsed: stored.sidebarCollapsed === true,
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

/** The longest place (allowed path or drive) that contains `path` */
export function placeOf(path: string, roots: string[]): string | null {
  let best: string | null = null;
  for (const root of roots) {
    const r = root.replace(/\/+$/, '') || '/';
    const inside = path === r || path.startsWith(r === '/' ? '/' : `${r}/`);
    if (inside && (!best || r.length > best.length)) best = r;
  }
  return best;
}

export function parentOf(path: string): string {
  const parent = path.replace(/\/+$/, '').split('/').slice(0, -1).join('/');
  return parent || '/';
}

/** State of the Files page; kept while the user visits other pages */
class FileBrowser {
  path = $state('');
  entries = $state<DirectoryEntry[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  sizesLoading = $state(false);
  places = $state<FilePlaces | null>(null);
  placesLoading = $state(false);
  prefs = $state<Prefs>(loadPrefs());

  sorted = $derived(sortEntries(this.entries, this.prefs.sortBy, this.prefs.sortOrder));

  /** Every place a folder can belong to: allowed paths and mounted drives */
  roots = $derived([
    ...(this.places?.places.map((p) => p.path) ?? []),
    ...((this.places?.drives.map((d) => d.mountpoint).filter(Boolean) as string[]) ?? []),
  ]);
  root = $derived(this.path ? placeOf(this.path, this.roots) : null);

  #generation = 0;
  #sizes: AbortController | null = null;

  async open(path: string): Promise<boolean> {
    const generation = ++this.#generation;
    this.#sizes?.abort();
    this.loading = true;
    this.error = null;
    try {
      const result = await api.system.browse(path);
      // Only the last navigation may change the listing
      if (generation !== this.#generation) return false;
      this.path = result.path;
      this.entries = result.entries;
      this.loading = false;
      this.#loadSizes(result.entries);
      return true;
    } catch (e) {
      if (generation !== this.#generation) return false;
      this.path = path;
      this.entries = [];
      this.loading = false;
      this.error = e instanceof Error ? e.message : m.error_browse_path();
      return false;
    }
  }

  refresh(): Promise<boolean> | undefined {
    return this.path ? this.open(this.path) : undefined;
  }

  close(): void {
    this.#generation++;
    this.#sizes?.abort();
    this.path = '';
    this.entries = [];
    this.error = null;
    this.loading = false;
    this.sizesLoading = false;
  }

  #loadSizes(entries: DirectoryEntry[]): void {
    const dirs = entries.filter((e) => e.is_dir).map((e) => e.path);
    if (dirs.length === 0) return;
    const controller = new AbortController();
    this.#sizes = controller;
    this.sizesLoading = true;
    streamDirSizes(
      dirs,
      (batch) => {
        this.entries = this.entries.map((e) => (e.path in batch ? { ...e, size: batch[e.path] } : e));
      },
      { signal: controller.signal },
    )
      .catch(() => {})
      .finally(() => {
        if (this.#sizes === controller) this.sizesLoading = false;
      });
  }

  /** Take entries out of the listing right away (moved to the trash) */
  remove(paths: string[]): void {
    const gone = new Set(paths);
    this.entries = this.entries.filter((e) => !gone.has(e.path));
  }

  async loadPlaces(): Promise<void> {
    this.placesLoading = true;
    try {
      this.places = await api.files.places();
    } catch {
      // The sidebar shows what it had; the list still works
    } finally {
      this.placesLoading = false;
    }
  }

  async mount(drive: FileDrive): Promise<string | null> {
    const { mount_point } = await api.system.generateMountPoint(drive.uuid, drive.label ?? undefined);
    await api.system.mount(drive.uuid, mount_point);
    await this.loadPlaces();
    return mount_point;
  }

  async unmount(drive: FileDrive): Promise<void> {
    if (!drive.mountpoint) return;
    await api.system.unmount(drive.mountpoint);
    if (this.path && placeOf(this.path, [drive.mountpoint])) this.close();
    await this.loadPlaces();
  }

  #savePrefs(): void {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(this.prefs));
    } catch {
      // Private mode and the like
    }
  }

  sortBy(field: SortField): void {
    if (this.prefs.sortBy === field) {
      this.prefs.sortOrder = this.prefs.sortOrder === 'asc' ? 'desc' : 'asc';
    } else {
      this.prefs.sortBy = field;
      // Newest and biggest first are the useful starting points
      this.prefs.sortOrder = field === 'name' ? 'asc' : 'desc';
    }
    this.#savePrefs();
  }

  setViewMode(mode: ViewMode): void {
    this.prefs.viewMode = mode;
    this.#savePrefs();
  }

  toggleSidebar(): void {
    this.prefs.sidebarCollapsed = !this.prefs.sidebarCollapsed;
    this.#savePrefs();
  }
}

export const fileBrowser = new FileBrowser();

import { api, ApiError, errorFromText } from '$lib/api';
import type { FileFacts } from '$lib/types';
import * as m from '$lib/paraglide/messages.js';

export type UploadState =
  | 'queued'
  | 'starting'
  | 'uploading'
  | 'retrying'
  | 'conflict'
  | 'done'
  | 'skipped'
  | 'failed'
  | 'cancelled';
export type UploadChoice = 'replace' | 'keep_both' | 'skip';

export interface UploadItem {
  key: number;
  file: File;
  /** Folder the upload goes into */
  dir: string;
  /** Subfolder below `dir` (folder uploads) */
  relDir: string;
  state: UploadState;
  sent: number;
  /** Bytes per second, smoothed */
  speed: number;
  id?: string;
  choice?: 'replace' | 'keep_both';
  existing?: FileFacts;
  error?: string;
  /** Where the finished file ended up */
  path?: string;
}

const PARALLEL = 2;
/** Waits between retries after a network error, in seconds */
export const RETRY_DELAYS = [1, 2, 4, 8, 15, 30, 30, 30, 60, 60];
const OPEN = new Set<UploadState>(['queued', 'starting', 'uploading', 'retrying', 'conflict']);

export function isUploadOpen(item: Pick<UploadItem, 'state'>): boolean {
  return OPEN.has(item.state);
}

/** Identifies a file across page reloads, so the server can resume it */
export function fingerprint(file: Pick<File, 'name' | 'size' | 'lastModified'>): string {
  return `${file.name}|${file.size}|${file.lastModified}`;
}

/** "bytes 0-99/1000" for a chunk of `length` bytes at `offset` */
export function contentRange(offset: number, length: number, total: number): string {
  return `bytes ${offset}-${offset + length - 1}/${total}`;
}

/** Seconds left at the current speed, or null when unknown */
export function secondsLeft(item: Pick<UploadItem, 'sent' | 'speed' | 'file'>): number | null {
  if (item.speed <= 0) return null;
  return Math.max(0, Math.round((item.file.size - item.sent) / item.speed));
}

class NetworkError extends Error {}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Uploads of the file manager: a queue, two files at a time */
class Uploader {
  items = $state<UploadItem[]>([]);
  open = $derived(this.items.filter(isUploadOpen));
  conflict = $derived(this.items.find((i) => i.state === 'conflict') ?? null);
  /** Share of all open uploads' bytes sent, 0..1 */
  progress = $derived.by(() => {
    const total = this.open.reduce((s, i) => s + i.file.size, 0);
    return total > 0 ? this.open.reduce((s, i) => s + i.sent, 0) / total : 0;
  });

  #next = 1;
  #running = 0;
  #applyAll: UploadChoice | null = null;
  #requests = new Map<number, XMLHttpRequest>();
  #finished = new Set<(item: UploadItem) => void>();

  add(files: { file: File; relDir: string }[], dir: string): void {
    const added = files.map(({ file, relDir }) => ({
      key: this.#next++,
      file,
      dir,
      relDir,
      state: 'queued' as UploadState,
      sent: 0,
      speed: 0,
    }));
    this.items = [...this.items, ...added];
    this.#pump();
  }

  /** Called once for each upload that ends */
  onFinished(listener: (item: UploadItem) => void): () => void {
    this.#finished.add(listener);
    return () => this.#finished.delete(listener);
  }

  /** Answer a taken name; with `all`, later conflicts get the same answer */
  resolve(key: number, choice: UploadChoice, all: boolean): void {
    if (all) this.#applyAll = choice;
    const item = this.items.find((i) => i.key === key);
    if (!item || item.state !== 'conflict') return;
    this.#decide(item, choice);
    // Others waiting for the same answer
    if (all) for (const other of this.items) if (other.state === 'conflict') this.#decide(other, choice);
    this.#pump();
  }

  #decide(item: UploadItem, choice: UploadChoice): void {
    if (choice === 'skip') {
      this.#end(item, 'skipped');
    } else {
      item.choice = choice;
      item.state = 'queued';
    }
  }

  async cancel(key: number): Promise<void> {
    const item = this.items.find((i) => i.key === key);
    if (!item || !isUploadOpen(item)) return;
    const wasRunning = item.state !== 'queued' && item.state !== 'conflict';
    this.#end(item, 'cancelled');
    this.#requests.get(key)?.abort();
    if (item.id) await api.files.cancelUpload(item.id).catch(() => {});
    if (!wasRunning) this.#pump();
  }

  /** Try a failed upload again (it resumes where it stopped) */
  retry(key: number): void {
    const item = this.items.find((i) => i.key === key);
    if (!item || item.state !== 'failed') return;
    item.state = 'queued';
    item.error = undefined;
    this.#pump();
  }

  clearFinished(): void {
    this.items = this.items.filter(isUploadOpen);
  }

  #end(item: UploadItem, state: UploadState): void {
    item.state = state;
    item.speed = 0;
    for (const listener of this.#finished) listener(item);
  }

  #pump(): void {
    while (this.#running < PARALLEL) {
      const item = this.items.find((i) => i.state === 'queued');
      if (!item) return;
      this.#running++;
      item.state = 'starting';
      this.#run(item).finally(() => {
        this.#running--;
        this.#pump();
      });
    }
  }

  async #run(item: UploadItem): Promise<void> {
    try {
      let started;
      try {
        started = await api.files.startUpload({
          dir: item.dir,
          rel_dir: item.relDir || undefined,
          name: item.file.name,
          size: item.file.size,
          mtime: item.file.lastModified || undefined,
          fingerprint: fingerprint(item.file),
          on_conflict: item.choice,
        });
      } catch (e) {
        if (e instanceof ApiError && e.code === 'NAME_TAKEN') {
          if (this.#applyAll) {
            this.#decide(item, this.#applyAll);
            return;
          }
          item.existing = e.params as unknown as FileFacts;
          item.state = 'conflict';
          return;
        }
        throw e;
      }
      if (!isUploadOpen(item)) return;
      item.id = started.id;
      if (started.path) {
        item.path = started.path;
        item.sent = item.file.size;
        this.#end(item, 'done');
        return;
      }

      let offset = started.offset;
      let attempt = 0;
      item.state = 'uploading';
      while (offset < item.file.size) {
        if (!isUploadOpen(item)) return;
        try {
          const result = await this.#send(item, offset, started.chunk_size);
          offset = result.offset;
          attempt = 0;
          item.state = 'uploading';
          if (result.path) item.path = result.path;
        } catch (e) {
          if (!isUploadOpen(item)) return;
          if (e instanceof ApiError && e.code === 'UPLOAD_OFFSET_MISMATCH' && typeof e.params?.offset === 'number') {
            offset = e.params.offset;
            continue;
          }
          const retryable = e instanceof NetworkError || (e instanceof ApiError && (e.status >= 500 || e.code === 'UPLOAD_BUSY'));
          if (!retryable || attempt >= RETRY_DELAYS.length) throw e;
          // Connection lost: wait, then ask the server where it stands
          item.state = 'retrying';
          item.speed = 0;
          await sleep(RETRY_DELAYS[attempt++] * 1000);
          if (!isUploadOpen(item)) return;
          try {
            offset = (await api.files.uploadStatus(item.id!)).offset;
          } catch {
            // Still offline: the next try waits longer
          }
        }
      }
      item.sent = item.file.size;
      this.#end(item, 'done');
    } catch (e) {
      if (!isUploadOpen(item)) return;
      item.error = e instanceof Error ? e.message : m.error_generic();
      this.#end(item, 'failed');
    }
  }

  /** Send one chunk; resolves with the new offset */
  #send(item: UploadItem, offset: number, chunk: number): Promise<{ offset: number; path?: string }> {
    const end = Math.min(item.file.size, offset + chunk);
    const url = api.files.uploadUrl(item.id!);
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      this.#requests.set(item.key, xhr);
      const began = performance.now();
      xhr.open('PUT', url);
      xhr.setRequestHeader('Content-Range', contentRange(offset, end - offset, item.file.size));
      xhr.upload.onprogress = (e) => {
        item.sent = offset + e.loaded;
        const seconds = (performance.now() - began) / 1000;
        if (seconds > 0.5) {
          const now = e.loaded / seconds;
          item.speed = item.speed > 0 ? item.speed * 0.7 + now * 0.3 : now;
        }
      };
      xhr.onload = () => {
        this.#requests.delete(item.key);
        if (xhr.status >= 200 && xhr.status < 300) {
          try {
            resolve(JSON.parse(xhr.responseText));
          } catch {
            reject(new NetworkError('bad response'));
          }
        } else {
          reject(errorFromText(xhr.status, xhr.responseText, '/files/uploads'));
        }
      };
      xhr.onerror = () => {
        this.#requests.delete(item.key);
        reject(new NetworkError('network'));
      };
      xhr.onabort = () => {
        this.#requests.delete(item.key);
        reject(new NetworkError('aborted'));
      };
      xhr.send(item.file.slice(offset, end));
    });
  }
}

export const uploader = new Uploader();

/** Files of a drop, with folders walked (folder uploads keep their structure) */
export async function filesFromDrop(data: DataTransfer): Promise<{ file: File; relDir: string }[]> {
  const entries = [...data.items]
    .map((item) => (item.kind === 'file' ? item.webkitGetAsEntry?.() : null))
    .filter((e): e is FileSystemEntry => !!e);
  if (entries.length === 0) return [...data.files].map((file) => ({ file, relDir: '' }));
  const out: { file: File; relDir: string }[] = [];
  async function walk(entry: FileSystemEntry, dir: string): Promise<void> {
    if (entry.isFile) {
      const file = await new Promise<File>((res, rej) => (entry as FileSystemFileEntry).file(res, rej));
      out.push({ file, relDir: dir });
    } else if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      const sub = dir ? `${dir}/${entry.name}` : entry.name;
      // readEntries returns the folder in batches
      for (;;) {
        const batch = await new Promise<FileSystemEntry[]>((res, rej) => reader.readEntries(res, rej));
        if (batch.length === 0) break;
        for (const child of batch) await walk(child, sub);
      }
    }
  }
  for (const entry of entries) await walk(entry, '');
  return out;
}

/** Files picked with a folder picker (`webkitdirectory`) */
export function filesFromFolderInput(files: FileList): { file: File; relDir: string }[] {
  return [...files].map((file) => {
    const rel = (file as File & { webkitRelativePath?: string }).webkitRelativePath ?? '';
    return { file, relDir: rel.split('/').slice(0, -1).join('/') };
  });
}

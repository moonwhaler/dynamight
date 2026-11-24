import { api } from '$lib/api';
import { statusStore, type FileTaskMessage } from '$lib/stores/logs';
import type { ConflictChoice, FileTask, FileTaskKind } from '$lib/types';

const OPEN_STATES = new Set(['queued', 'running', 'conflict']);
const POLL_MS = 5000;

export function isOpen(task: FileTask): boolean {
  return OPEN_STATES.has(task.state);
}

/** Share of all open tasks' bytes done, 0..1 (files count when there are no bytes) */
export function overallProgress(tasks: FileTask[]): number {
  const open = tasks.filter(isOpen);
  const total = open.reduce((s, t) => s + t.total_bytes, 0);
  if (total > 0) return Math.min(1, open.reduce((s, t) => s + t.done_bytes, 0) / total);
  const files = open.reduce((s, t) => s + t.total_files, 0);
  return files > 0 ? Math.min(1, open.reduce((s, t) => s + t.done_files, 0) / files) : 0;
}

/** Copy and move tasks of the file manager, kept live over the status socket */
class FileTasks {
  list = $state<FileTask[]>([]);
  open = $derived(this.list.filter(isOpen));
  conflict = $derived(this.list.find((t) => t.state === 'conflict') ?? null);
  progress = $derived(overallProgress(this.list));

  #users = 0;
  #unsubscribe: (() => void) | null = null;
  #finished = new Set<(task: FileTask) => void>();
  /** Finished tasks the user cleared (the server lists them a while longer) */
  #cleared = new Set<number>();
  #poll: ReturnType<typeof setInterval> | null = null;

  /** Start receiving updates; returns the matching disconnect */
  connect(): () => void {
    if (this.#users++ === 0) {
      this.#unsubscribe = statusStore.onFileTask((message) => this.#receive(message));
      statusStore.connect();
      this.load();
      // Backstop for a socket that is down: poll while something runs
      this.#poll = setInterval(() => {
        if (this.open.length > 0) this.load();
      }, POLL_MS);
    }
    let done = false;
    return () => {
      if (done) return;
      done = true;
      if (--this.#users === 0) {
        if (this.#poll) clearInterval(this.#poll);
        this.#poll = null;
        this.#unsubscribe?.();
        this.#unsubscribe = null;
        statusStore.disconnect();
      }
    };
  }

  /** Called once for each task that ends (done, failed or cancelled) */
  onFinished(listener: (task: FileTask) => void): () => void {
    this.#finished.add(listener);
    return () => this.#finished.delete(listener);
  }

  async load(): Promise<void> {
    try {
      const tasks = await api.files.tasks();
      for (const task of tasks) this.apply(task);
      const ids = new Set(tasks.map((t) => t.id));
      this.list = this.list.filter((t) => ids.has(t.id));
    } catch {
      // Keep what we have
    }
  }

  #receive(message: FileTaskMessage): void {
    if (message.type === 'file_tasks_resync') this.load();
    else this.apply(message.task);
  }

  apply(task: FileTask): void {
    if (this.#cleared.has(task.id)) return;
    const index = this.list.findIndex((t) => t.id === task.id);
    const before = index >= 0 ? this.list[index] : null;
    // The HTTP answer and socket events can overtake each other
    if (before && task.revision < before.revision) return;
    if (before && !isOpen(before) && isOpen(task)) return;
    if (index >= 0) this.list[index] = task;
    else this.list = [...this.list, task];
    // Only an end seen happen (not old tasks listed on page load)
    if (before && isOpen(before) && !isOpen(task)) {
      for (const listener of this.#finished) listener(task);
    }
  }

  async start(kind: FileTaskKind, sources: string[], target: string): Promise<FileTask> {
    const task = await api.files.startTask(kind, sources, target);
    this.apply(task);
    return task;
  }

  async cancel(id: number): Promise<void> {
    await api.files.cancelTask(id);
  }

  async resolve(id: number, choice: ConflictChoice, applyToAll: boolean): Promise<void> {
    await api.files.resolveTask(id, choice, applyToAll);
  }

  /** Remove finished tasks from the list */
  clearFinished(): void {
    for (const task of this.list) if (!isOpen(task)) this.#cleared.add(task.id);
    this.list = this.list.filter(isOpen);
  }
}

export const fileTasks = new FileTasks();

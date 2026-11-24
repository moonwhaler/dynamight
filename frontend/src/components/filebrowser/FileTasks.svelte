<script lang="ts">
  import { fileTasks, isOpen } from '$lib/files/tasks.svelte';
  import { uploader, isUploadOpen, secondsLeft, type UploadItem } from '$lib/files/uploader.svelte';
  import type { FileTask } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import Icon from '../ui/Icon.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let open = $state(false);
  let panel = $state<HTMLDivElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);

  const tasks = fileTasks;
  const ring = $derived(2 * Math.PI * 9);
  const anything = $derived(tasks.list.length + uploader.items.length > 0);
  const openCount = $derived(tasks.open.length + uploader.open.length);
  const waiting = $derived(!!tasks.conflict || !!uploader.conflict);
  /** Tasks and uploads together, weighted by how many are open */
  const progress = $derived.by(() => {
    const parts = [
      ...(tasks.open.length ? [tasks.progress] : []),
      ...(uploader.open.length ? [uploader.progress] : []),
    ];
    return parts.length ? parts.reduce((a, b) => a + b, 0) / parts.length : 0;
  });
  const finishedCount = $derived(tasks.list.length - tasks.open.length + uploader.items.length - uploader.open.length);

  function clearAll() {
    tasks.clearFinished();
    uploader.clearFinished();
  }

  function uploadStatus(item: UploadItem): string {
    switch (item.state) {
      case 'queued':
      case 'starting':
        return m.filebrowser_upload_waiting();
      case 'retrying':
        return m.filebrowser_upload_retrying();
      case 'conflict':
        return m.filebrowser_task_conflict();
      case 'done':
        return m.filebrowser_upload_done();
      case 'skipped':
        return m.filebrowser_upload_skipped();
      case 'cancelled':
        return m.filebrowser_task_cancelled();
      case 'failed':
        return item.error ?? m.error_file_operation_failed();
      default: {
        const left = secondsLeft(item);
        const bytes = m.filebrowser_task_bytes({ done: formatBytes(item.sent), total: formatBytes(item.file.size) });
        if (item.speed <= 0) return bytes;
        const time = left === null ? '' : left >= 60 ? m.filebrowser_minutes_left({ count: Math.ceil(left / 60) }) : m.filebrowser_seconds_left({ count: left });
        return `${bytes} · ${formatBytes(item.speed)}/s${time ? ` · ${time}` : ''}`;
      }
    }
  }

  function name(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path;
  }

  function title(task: FileTask): string {
    const what = task.sources.length === 1 ? name(task.sources[0]) : m.filebrowser_items({ count: task.sources.length });
    const where = name(task.target);
    return task.kind === 'copy' ? m.filebrowser_task_copy({ what, where }) : m.filebrowser_task_move({ what, where });
  }

  function status(task: FileTask): string {
    switch (task.state) {
      case 'queued':
        return m.filebrowser_task_queued();
      case 'conflict':
        return m.filebrowser_task_conflict();
      case 'done':
        return task.errors.length > 0 ? m.filebrowser_task_done_errors({ count: task.errors.length }) : m.filebrowser_task_done();
      case 'failed':
        return task.failure ?? m.error_file_operation_failed();
      case 'cancelled':
        return m.filebrowser_task_cancelled();
      default:
        return task.total_bytes > 0
          ? m.filebrowser_task_bytes({ done: formatBytes(task.done_bytes), total: formatBytes(task.total_bytes) })
          : m.filebrowser_task_files({ done: task.done_files, total: task.total_files });
    }
  }

  function share(task: FileTask): number {
    if (!isOpen(task)) return 1;
    if (task.total_bytes > 0) return task.done_bytes / task.total_bytes;
    return task.total_files > 0 ? task.done_files / task.total_files : 0;
  }

  async function cancel(task: FileTask) {
    try {
      await tasks.cancel(task.id);
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    }
  }

  $effect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => {
      if (!panel?.contains(e.target as Node) && !button?.contains(e.target as Node)) open = false;
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        open = false;
        button?.focus();
      }
    };
    document.addEventListener('mousedown', outside);
    document.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('mousedown', outside);
      document.removeEventListener('keydown', key);
    };
  });
</script>

{#if anything}
  <div class="relative shrink-0">
    <button
      bind:this={button}
      type="button"
      class="btn btn-ghost btn-icon btn-sm relative"
      aria-expanded={open}
      aria-haspopup="dialog"
      aria-label={m.filebrowser_tasks()}
      title={m.filebrowser_tasks()}
      onclick={() => (open = !open)}
    >
      {#if openCount > 0}
        <svg class="w-6 h-6 -rotate-90" viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="9" fill="none" stroke-width="2.5" class="stroke-gray-200 dark:stroke-gray-700" />
          <circle
            cx="12"
            cy="12"
            r="9"
            fill="none"
            stroke-width="2.5"
            stroke-linecap="round"
            class="{waiting ? 'stroke-amber-500' : 'stroke-primary-500'} transition-[stroke-dashoffset] duration-300"
            stroke-dasharray={ring}
            stroke-dashoffset={ring * (1 - progress)}
          />
        </svg>
      {:else}
        <Icon name="check-circle" class="w-5 h-5 text-green-600 dark:text-green-400" />
      {/if}
    </button>

    {#if open}
      <div
        bind:this={panel}
        role="dialog"
        aria-label={m.filebrowser_tasks()}
        class="absolute right-0 top-full mt-2 z-40 w-[min(24rem,calc(100vw-2rem))] rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-xl"
      >
        <div class="flex items-center justify-between px-4 py-2.5 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white">{m.filebrowser_tasks()}</h2>
          {#if finishedCount > 0}
            <button type="button" class="text-xs text-primary-700 dark:text-primary-300 hover:underline" onclick={clearAll}>
              {m.filebrowser_tasks_clear()}
            </button>
          {/if}
        </div>
        <ul class="max-h-96 overflow-y-auto divide-y divide-gray-100 dark:divide-gray-700/60">
          {#each [...uploader.items].reverse() as item (item.key)}
            <li class="px-4 py-3">
              <div class="flex items-start gap-2">
                <Icon
                  name={item.state === 'failed' ? 'x-circle' : item.state === 'done' ? 'check-circle' : 'upload'}
                  class="w-4 h-4 mt-0.5 shrink-0 {item.state === 'failed'
                    ? 'text-red-500'
                    : item.state === 'done'
                      ? 'text-green-600 dark:text-green-400'
                      : item.state === 'retrying'
                        ? 'text-amber-500'
                        : 'text-gray-400'}"
                />
                <div class="min-w-0 flex-1">
                  <div class="text-sm text-gray-900 dark:text-white truncate" title={item.relDir ? `${item.relDir}/${item.file.name}` : item.file.name}>
                    {item.file.name}
                  </div>
                  <div class="text-xs text-gray-500 dark:text-gray-400 tabular-nums {item.state === 'failed' ? '!text-red-600 dark:!text-red-400' : ''}">
                    {uploadStatus(item)}
                  </div>
                  {#if isUploadOpen(item) && item.state !== 'conflict'}
                    <div class="mt-2 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                      <div
                        class="h-full rounded-full {item.state === 'retrying' ? 'bg-amber-500' : 'bg-primary-500'}"
                        style="width: {item.file.size ? Math.round((item.sent / item.file.size) * 100) : 0}%"
                      ></div>
                    </div>
                  {/if}
                </div>
                {#if isUploadOpen(item)}
                  <button type="button" class="btn btn-ghost btn-sm shrink-0 -mr-2" onclick={() => uploader.cancel(item.key)}>{m.common_cancel()}</button>
                {:else if item.state === 'failed'}
                  <button type="button" class="btn btn-ghost btn-sm shrink-0 -mr-2" onclick={() => uploader.retry(item.key)}>{m.common_retry()}</button>
                {/if}
              </div>
            </li>
          {/each}
          {#each [...tasks.list].reverse() as task (task.id)}
            <li class="px-4 py-3">
              <div class="flex items-start gap-2">
                <Icon
                  name={task.state === 'failed' ? 'x-circle' : task.state === 'done' ? 'check-circle' : task.kind === 'copy' ? 'copy' : 'arrows'}
                  class="w-4 h-4 mt-0.5 shrink-0 {task.state === 'failed'
                    ? 'text-red-500'
                    : task.state === 'done'
                      ? task.errors.length
                        ? 'text-amber-500'
                        : 'text-green-600 dark:text-green-400'
                      : 'text-gray-400'}"
                />
                <div class="min-w-0 flex-1">
                  <div class="text-sm text-gray-900 dark:text-white truncate" title={task.sources.join('\n')}>{title(task)}</div>
                  <div class="text-xs text-gray-500 dark:text-gray-400 tabular-nums {task.state === 'failed' ? '!text-red-600 dark:!text-red-400' : ''}">
                    {status(task)}
                  </div>
                  {#if isOpen(task)}
                    <div class="mt-2 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                      <div class="h-full rounded-full {task.state === 'conflict' ? 'bg-amber-500' : 'bg-primary-500'}" style="width: {Math.round(share(task) * 100)}%"></div>
                    </div>
                    {#if task.current}
                      <div class="mt-1 text-[11px] text-gray-400 truncate" title={task.current}>{name(task.current)}</div>
                    {/if}
                  {/if}
                  {#if task.errors.length > 0}
                    <details class="mt-1.5 text-xs">
                      <summary class="cursor-pointer text-amber-700 dark:text-amber-400">{m.filebrowser_task_show_errors()}</summary>
                      <ul class="mt-1 space-y-1 text-gray-600 dark:text-gray-400">
                        {#each task.errors as err, i (i)}
                          <li class="break-words"><span class="font-mono">{name(err.path)}</span>: {err.message}</li>
                        {/each}
                      </ul>
                    </details>
                  {/if}
                </div>
                {#if isOpen(task)}
                  <button type="button" class="btn btn-ghost btn-sm shrink-0 -mr-2" onclick={() => cancel(task)}>{m.common_cancel()}</button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
{/if}

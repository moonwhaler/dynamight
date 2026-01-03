<script lang="ts">
  import type { LogEntry } from '../../lib/types';
  import { tick } from 'svelte';

  let { logs = [] }: { logs: LogEntry[] } = $props();

  let container: HTMLDivElement;
  let autoScroll = $state(true);

  // Track logs.length to trigger scroll after new entries
  $effect(() => {
    const _len = logs.length; // Create dependency on logs.length
    if (autoScroll && container && _len > 0) {
      // Wait for DOM update before scrolling
      tick().then(() => {
        if (container) {
          container.scrollTop = container.scrollHeight;
        }
      });
    }
  });

  function getLevelClass(level: string): string {
    switch (level) {
      case 'error':
        return 'text-red-400';
      case 'warning':
        return 'text-yellow-400';
      case 'debug':
        return 'text-gray-500';
      default:
        return 'text-gray-400';
    }
  }

  function formatTime(timestamp: string): string {
    return new Date(timestamp).toLocaleTimeString();
  }
</script>

<!-- Header bar - fixed height -->
<div class="p-2 border-b flex items-center justify-between bg-gray-50">
  <span class="text-sm text-gray-500">{logs.length} entries</span>
  <label class="flex items-center gap-2 text-sm cursor-pointer">
    <input type="checkbox" bind:checked={autoScroll} class="rounded text-primary-600" />
    Auto-scroll
  </label>
</div>

<!-- Log content - takes remaining height via calc -->
<div
  bind:this={container}
  class="overflow-y-auto bg-gray-900 p-4 font-mono text-sm"
  style="height: calc(100% - 41px);"
>
  {#if logs.length === 0}
    <p class="text-gray-500">Waiting for logs...</p>
  {:else}
    {#each logs as entry (entry.id)}
      <div class="flex gap-2 py-0.5 hover:bg-gray-800">
        <span class="text-gray-500 shrink-0 tabular-nums">{formatTime(entry.timestamp)}</span>
        <span
          class="shrink-0 w-14 uppercase text-xs font-semibold {getLevelClass(entry.level)}"
        >
          {entry.level}
        </span>
        {#if entry.source}
          <span class="text-purple-400 shrink-0">[{entry.source}]</span>
        {/if}
        <span class="text-gray-200 break-all whitespace-pre-wrap">{entry.message}</span>
      </div>
    {/each}
  {/if}
</div>

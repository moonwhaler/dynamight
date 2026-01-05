<script lang="ts">
  import type { LogEntry } from '../../lib/types';
  import { onDestroy, tick } from 'svelte';
  import { get } from 'svelte/store';
  import { preferencesStore } from '../../lib/stores/preferences';

  interface Props {
    logs: LogEntry[];
    total?: number;
    currentPage?: number;
    totalPages?: number;
    loading?: boolean;
    pageSize?: number;
    onPageChange?: (page: number) => void;
    /** For live streaming mode - new entries append to logs array */
    isStreaming?: boolean;
  }

  let {
    logs = [],
    total = 0,
    currentPage = 1,
    totalPages = 1,
    loading = false,
    pageSize = 500,
    onPageChange,
    isStreaming = false
  }: Props = $props();

  let container: HTMLDivElement;
  let autoScrollEnabled = $state(get(preferencesStore).autoShowLastPage);
  let jumpToPage = $state('');

  // Track if user has manually scrolled away from bottom
  let userScrolledAway = $state(false);

  // Subscribe to preference changes
  const unsubscribe = preferencesStore.subscribe((prefs) => {
    autoScrollEnabled = prefs.autoShowLastPage;
    if (autoScrollEnabled && isStreaming) {
      userScrolledAway = false;
      scrollToBottom();
    }
  });

  onDestroy(() => {
    unsubscribe();
  });

  function toggleAutoScroll(checked: boolean) {
    autoScrollEnabled = checked;
    preferencesStore.setAutoShowLastPage(checked);
    if (checked) {
      userScrolledAway = false;
      scrollToBottom();
    }
  }

  async function scrollToBottom() {
    await tick();
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
  }

  function handleScroll() {
    if (!container || !isStreaming) return;

    // Check if user is at the bottom (within 50px threshold)
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 50;

    if (autoScrollEnabled) {
      // If user scrolled away from bottom, pause auto-scroll
      userScrolledAway = !isAtBottom;
    }
  }

  function jumpToBottom() {
    userScrolledAway = false;
    scrollToBottom();
  }

  // Auto-scroll when new logs arrive (streaming mode)
  $effect(() => {
    if (isStreaming && autoScrollEnabled && !userScrolledAway && logs.length > 0) {
      scrollToBottom();
    }
  });

  // For non-streaming mode: pagination
  function goToPage(page: number) {
    if (page < 1 || page > totalPages) return;
    if (page !== currentPage && onPageChange) {
      onPageChange(page);
    }
  }

  function handleJumpToPage() {
    const page = parseInt(jumpToPage, 10);
    if (!isNaN(page)) {
      goToPage(page);
      jumpToPage = '';
    }
  }

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

  // Calculate display range for non-streaming mode
  const startEntry = $derived((currentPage - 1) * pageSize + 1);
  const endEntry = $derived(Math.min(currentPage * pageSize, total));
</script>

<!-- Header bar - fixed height -->
<div class="p-2 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between bg-gray-50 dark:bg-gray-800 gap-2 flex-wrap">
  <span class="text-sm text-gray-500 dark:text-gray-400">
    {#if isStreaming}
      {logs.length.toLocaleString()} entries (live)
    {:else if total > 0}
      {startEntry.toLocaleString()}-{endEntry.toLocaleString()} of {total.toLocaleString()} entries
    {:else}
      0 entries
    {/if}
  </span>

  <div class="flex items-center gap-3">
    {#if isStreaming}
      <label class="flex items-center gap-2 text-sm cursor-pointer text-gray-700 dark:text-gray-300">
        <input type="checkbox" checked={autoScrollEnabled} onchange={(e) => toggleAutoScroll(e.currentTarget.checked)} class="rounded text-primary-600" />
        Auto-scroll
      </label>
      {#if userScrolledAway && autoScrollEnabled}
        <button
          onclick={jumpToBottom}
          class="px-2 py-1 text-xs bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300 rounded hover:bg-primary-200 dark:hover:bg-primary-900/60 flex items-center gap-1"
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 14l-7 7m0 0l-7-7m7 7V3" />
          </svg>
          Jump to latest
        </button>
      {/if}
    {:else if totalPages > 1}
      <!-- Pagination controls for non-streaming mode -->
      <div class="flex items-center gap-1 text-sm">
        <button
          onclick={() => goToPage(1)}
          disabled={currentPage === 1 || loading}
          class="px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed text-gray-700 dark:text-gray-300"
          title="First page"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
          </svg>
        </button>
        <button
          onclick={() => goToPage(currentPage - 1)}
          disabled={currentPage === 1 || loading}
          class="px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed text-gray-700 dark:text-gray-300"
          title="Previous page"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>

        <span class="px-2 text-gray-600 dark:text-gray-400">
          Page {currentPage} of {totalPages}
        </span>

        <button
          onclick={() => goToPage(currentPage + 1)}
          disabled={currentPage === totalPages || loading}
          class="px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed text-gray-700 dark:text-gray-300"
          title="Next page"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </button>
        <button
          onclick={() => goToPage(totalPages)}
          disabled={currentPage === totalPages || loading}
          class="px-2 py-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed text-gray-700 dark:text-gray-300"
          title="Last page"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
          </svg>
        </button>

        <div class="flex items-center gap-1 ml-2">
          <input
            type="number"
            bind:value={jumpToPage}
            placeholder="#"
            min="1"
            max={totalPages}
            class="w-14 px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded focus:outline-none focus:ring-1 focus:ring-primary-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
            onkeydown={(e) => e.key === 'Enter' && handleJumpToPage()}
          />
          <button
            onclick={handleJumpToPage}
            disabled={loading}
            class="px-2 py-1 text-xs bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 disabled:opacity-40"
          >
            Go
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

<!-- Log content - takes remaining height via calc -->
<div
  bind:this={container}
  class="overflow-y-auto bg-gray-900 p-4 font-mono text-sm"
  style="height: calc(100% - 49px);"
  onscroll={handleScroll}
>
  {#if loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-400"></div>
    </div>
  {:else if logs.length === 0}
    <p class="text-gray-500">No log entries.</p>
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

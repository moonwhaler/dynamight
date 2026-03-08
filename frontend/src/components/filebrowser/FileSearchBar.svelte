<script lang="ts">
  import { slide } from 'svelte/transition';
  import type { SearchMode } from '$lib/types';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    query: string;
    matchCount: number;
    totalCount: number;
    onClose: () => void;
    searchMode?: SearchMode;
    onSearchModeChange?: (mode: SearchMode) => void;
    onSearch?: () => void;
    deepSearchLoading?: boolean;
    deepSearchTruncated?: boolean;
    deepSearchTimedOut?: boolean;
    deepSearchCount?: number;
  }

  let {
    query = $bindable(),
    matchCount,
    totalCount,
    onClose,
    searchMode = 'local',
    onSearchModeChange,
    onSearch,
    deepSearchLoading = false,
    deepSearchTruncated = false,
    deepSearchTimedOut = false,
    deepSearchCount = 0,
  }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    } else if (e.key === 'Enter' && searchMode === 'deep' && onSearch) {
      e.preventDefault();
      onSearch();
    }
  }
</script>

<div transition:slide={{ duration: 200 }} class="px-4 py-2 border-b border-gray-100 dark:border-gray-700">
  <div class="flex items-center gap-2">
    <!-- Search icon -->
    <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>

    <!-- Search mode toggle -->
    {#if onSearchModeChange}
      <div class="flex items-center rounded-md bg-gray-100 dark:bg-gray-700/50 p-0.5 text-xs flex-shrink-0">
        <button
          type="button"
          onclick={() => onSearchModeChange('local')}
          class="px-2 py-0.5 rounded transition-colors {searchMode === 'local'
            ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow-sm'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
        >
          {m.filebrowser_search_this_folder()}
        </button>
        <button
          type="button"
          onclick={() => onSearchModeChange('deep')}
          class="px-2 py-0.5 rounded transition-colors {searchMode === 'deep'
            ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow-sm'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
        >
          {m.filebrowser_search_subfolders()}
        </button>
      </div>
    {/if}

    <!-- Input -->
    <input
      bind:this={inputEl}
      bind:value={query}
      type="text"
      class="flex-1 bg-transparent border-none outline-none text-sm text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-gray-500"
      placeholder={searchMode === 'deep' ? m.filebrowser_search_deep_placeholder() : m.filebrowser_search_placeholder()}
      onkeydown={handleKeydown}
      autocomplete="off"
      spellcheck="false"
    />

    <!-- Loading spinner for deep search -->
    {#if deepSearchLoading}
      <svg class="w-4 h-4 animate-spin text-primary-500 flex-shrink-0" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
      </svg>
    {/if}

    <!-- Match count / status badges -->
    {#if searchMode === 'deep' && query.trim().length > 0 && query.trim().length < 2}
      <span class="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
        {m.filebrowser_search_deep_min_chars()}
      </span>
    {:else if searchMode === 'deep' && deepSearchLoading}
      <!-- spinner is already shown, no extra badge needed -->
    {:else if searchMode === 'deep' && deepSearchTimedOut}
      <span class="text-xs text-red-500 dark:text-red-400 whitespace-nowrap">
        {m.filebrowser_search_timed_out()}
      </span>
    {:else if searchMode === 'deep' && deepSearchTruncated}
      <span class="text-xs text-amber-500 dark:text-amber-400 whitespace-nowrap">
        {m.filebrowser_search_truncated({ count: deepSearchCount })}
      </span>
    {:else if searchMode === 'deep' && deepSearchCount > 0}
      <span class="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
        {deepSearchCount}
      </span>
    {:else if searchMode === 'deep' && query.trim().length >= 2}
      <span class="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
        {m.filebrowser_search_press_enter()}
      </span>
    {:else if searchMode === 'local' && query.trim()}
      <span class="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
        {m.filebrowser_search_results({ count: matchCount, total: totalCount })}
      </span>
    {/if}

    <!-- Clear / close button -->
    <button
      type="button"
      onclick={onClose}
      class="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 flex-shrink-0"
      title={m.filebrowser_search_clear()}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
</div>

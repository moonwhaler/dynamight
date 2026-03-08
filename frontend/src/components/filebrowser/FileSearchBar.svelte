<script lang="ts">
  import { slide } from 'svelte/transition';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    query: string;
    matchCount: number;
    totalCount: number;
    onClose: () => void;
  }

  let { query = $bindable(), matchCount, totalCount, onClose }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }
</script>

<div transition:slide={{ duration: 200 }} class="px-4 py-2 border-b border-gray-100 dark:border-gray-700">
  <div class="flex items-center gap-2">
    <!-- Search icon -->
    <svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>

    <!-- Input -->
    <input
      bind:this={inputEl}
      bind:value={query}
      type="text"
      class="flex-1 bg-transparent border-none outline-none text-sm text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-gray-500"
      placeholder={m.filebrowser_search_placeholder()}
      onkeydown={handleKeydown}
      autocomplete="off"
      spellcheck="false"
    />

    <!-- Match count badge -->
    {#if query.trim()}
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

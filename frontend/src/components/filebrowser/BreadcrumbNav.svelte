<script lang="ts">
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    path: string;
    allowedPaths: string[];
    onNavigate: (path: string) => void;
    onGoUp?: () => void;
    onGoBack?: () => void;
    canGoBack?: boolean;
  }

  let { path, allowedPaths, onNavigate, onGoUp, onGoBack, canGoBack = false }: Props = $props();

  // Split path into segments for breadcrumb
  const pathSegments = $derived.by(() => {
    if (!path) return [];
    const parts = path.split('/').filter(Boolean);
    return parts.map((part, index) => ({
      name: part,
      path: '/' + parts.slice(0, index + 1).join('/'),
    }));
  });

  // Check if current path is a root allowed path
  const isAtRoot = $derived(allowedPaths.includes(path));
</script>

<div class="flex items-center gap-2 text-sm overflow-hidden">
  <!-- Back button (mobile friendly) -->
  {#if onGoBack && canGoBack}
    <button
      type="button"
      onclick={onGoBack}
      class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 flex-shrink-0"
      title={m.filebrowser_go_back()}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
      </svg>
    </button>
  {/if}

  <!-- Up button -->
  {#if onGoUp && !isAtRoot}
    <button
      type="button"
      onclick={onGoUp}
      class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 flex-shrink-0"
      title={m.filebrowser_go_up()}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 10l7-7m0 0l7 7m-7-7v18" />
      </svg>
    </button>
  {/if}

  <!-- Home/Root button -->
  <button
    type="button"
    onclick={() => onNavigate('/')}
    class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-300 flex-shrink-0"
    title="Root"
  >
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
    </svg>
  </button>

  <!-- Breadcrumb path -->
  <div class="flex items-center gap-1 overflow-x-auto scrollbar-hide min-w-0 flex-1">
    {#each pathSegments as segment, i}
      <span class="text-gray-400 dark:text-gray-500 flex-shrink-0">/</span>
      {#if i === pathSegments.length - 1}
        <span class="font-medium text-gray-900 dark:text-white truncate">{segment.name}</span>
      {:else}
        <button
          type="button"
          onclick={() => onNavigate(segment.path)}
          class="text-gray-600 dark:text-gray-300 hover:text-primary-600 dark:hover:text-primary-400 hover:underline truncate max-w-[120px]"
        >
          {segment.name}
        </button>
      {/if}
    {/each}

    {#if pathSegments.length === 0}
      <span class="text-gray-500 dark:text-gray-400">/</span>
    {/if}
  </div>
</div>

<style>
  .scrollbar-hide {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
  .scrollbar-hide::-webkit-scrollbar {
    display: none;
  }
</style>

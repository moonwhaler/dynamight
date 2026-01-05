<script lang="ts">
  import { location } from 'svelte-spa-router';
  import * as m from '$lib/paraglide/messages.js';

  let { open = false, onClose }: { open?: boolean; onClose?: () => void } = $props();

  const navItems = [
    { path: '/', labelKey: 'dashboard', icon: 'home' },
    { path: '/jobs', labelKey: 'jobs', icon: 'folder' },
    { path: '/history', labelKey: 'history', icon: 'clock' },
    { path: '/files', labelKey: 'files', icon: 'document' },
  ];

  function getNavLabel(key: string): string {
    switch (key) {
      case 'dashboard': return m.nav_dashboard();
      case 'jobs': return m.nav_jobs();
      case 'history': return m.nav_history();
      case 'files': return m.nav_files();
      default: return key;
    }
  }

  function isActive(path: string, current: string): boolean {
    if (path === '/') return current === '/';
    return current.startsWith(path);
  }

  function handleNavClick() {
    // Close sidebar on mobile after navigation
    onClose?.();
  }
</script>

<!-- Sidebar: hidden on mobile by default, shown as overlay when open -->
<aside
  class="
    fixed md:static inset-y-0 left-0 z-50 md:z-auto
    w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 p-4
    transform transition-transform duration-200 ease-in-out
    {open ? 'translate-x-0' : '-translate-x-full'} md:translate-x-0
    top-0 md:top-auto h-full md:h-auto
    flex flex-col
  "
>
  <!-- Mobile close button -->
  <div class="flex items-center justify-between mb-4 md:hidden">
    <span class="text-lg font-semibold text-gray-900 dark:text-white">{m.sidebar_menu()}</span>
    <button
      onclick={onClose}
      class="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 rounded-lg"
      aria-label={m.sidebar_close_menu()}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>

  <nav class="space-y-1 flex-1">
    {#each navItems as item}
      <a
        href="#{item.path}"
        onclick={handleNavClick}
        class="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors {isActive(
          item.path,
          $location
        )
          ? 'bg-primary-50 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400'
          : 'text-gray-600 hover:bg-gray-50 dark:text-gray-300 dark:hover:bg-gray-700/50'}"
      >
        {#if item.icon === 'home'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
            />
          </svg>
        {:else if item.icon === 'folder'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
            />
          </svg>
        {:else if item.icon === 'clock'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
        {:else if item.icon === 'document'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z"
            />
          </svg>
        {/if}
        <span class="font-medium">{getNavLabel(item.labelKey)}</span>
      </a>
    {/each}
  </nav>
</aside>

<script lang="ts">
  import { location } from 'svelte-spa-router';

  const navItems = [
    { path: '/', label: 'Dashboard', icon: 'home' },
    { path: '/jobs', label: 'Jobs', icon: 'folder' },
    { path: '/history', label: 'History', icon: 'clock' },
  ];

  function isActive(path: string, current: string): boolean {
    if (path === '/') return current === '/';
    return current.startsWith(path);
  }
</script>

<aside class="w-64 bg-white border-r border-gray-200 p-4">
  <nav class="space-y-1">
    {#each navItems as item}
      <a
        href="#{item.path}"
        class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors {isActive(
          item.path,
          $location
        )
          ? 'bg-primary-50 text-primary-700'
          : 'text-gray-600 hover:bg-gray-50'}"
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
        {/if}
        <span class="font-medium">{item.label}</span>
      </a>
    {/each}
  </nav>
</aside>

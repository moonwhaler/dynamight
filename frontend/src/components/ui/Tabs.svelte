<script lang="ts">
  // Navigation tabs: each tab is a link, the active one is marked.
  let { tabs, active }: { tabs: { id: string; label: string; href: string }[]; active: string } = $props();
</script>

<nav class="-mx-4 sm:mx-0 px-4 sm:px-0 overflow-x-auto scrollbar-hide border-b border-gray-200 dark:border-gray-700">
  <ul class="flex gap-6 min-w-max">
    {#each tabs as tab (tab.id)}
      <li>
        <a
          href={tab.href}
          aria-current={tab.id === active ? 'page' : undefined}
          class="block py-3 text-sm font-medium border-b-2 -mb-px transition-colors
            {tab.id === active
            ? 'border-primary-600 text-gray-900 dark:text-white'
            : 'border-transparent text-gray-500 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200'}"
        >
          {tab.label}
        </a>
      </li>
    {/each}
  </ul>
</nav>

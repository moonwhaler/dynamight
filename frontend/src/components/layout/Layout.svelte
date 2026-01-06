<script lang="ts">
  import Navbar from './Navbar.svelte';
  import Sidebar from './Sidebar.svelte';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();

  let sidebarOpen = $state(false);

  function toggleSidebar() {
    sidebarOpen = !sidebarOpen;
  }

  function closeSidebar() {
    sidebarOpen = false;
  }
</script>

<div class="h-screen flex flex-col overflow-hidden">
  <Navbar onMenuToggle={toggleSidebar} />

  <div class="flex flex-1 min-h-0 relative">
    <!-- Mobile backdrop -->
    {#if sidebarOpen}
      <button
        class="fixed inset-0 bg-black/50 z-40 md:hidden"
        onclick={closeSidebar}
        aria-label="Close sidebar"
      ></button>
    {/if}

    <Sidebar open={sidebarOpen} onClose={closeSidebar} />

    <main class="flex-1 p-4 sm:p-6 overflow-y-auto w-full">
      {@render children()}
    </main>
  </div>
</div>

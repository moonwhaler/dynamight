<script lang="ts">
  import Navbar from './Navbar.svelte';
  import Sidebar from './Sidebar.svelte';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="min-h-screen flex flex-col">
  <Navbar />

  <div class="flex flex-1">
    <Sidebar />

    <main class="flex-1 p-6 overflow-auto">
      {@render children()}
    </main>
  </div>
</div>

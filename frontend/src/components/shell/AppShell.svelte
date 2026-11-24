<script lang="ts">
  import type { Snippet } from 'svelte';
  import Sidebar from './Sidebar.svelte';
  import BottomTabs from './BottomTabs.svelte';
  import Logo from './Logo.svelte';
  import RunLogHost from '../logs/RunLogHost.svelte';
  import { loadServerTimezone } from '$lib/stores/serverTimezone';
  import * as m from '$lib/paraglide/messages.js';

  // Needed wherever schedule times are shown
  loadServerTimezone();

  let { children }: { children: Snippet } = $props();
</script>

<a
  href="#main"
  class="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[70] btn btn-primary"
  onclick={(e) => {
    e.preventDefault();
    document.getElementById('main')?.focus();
  }}>{m.nav_skip_to_content()}</a
>

<div class="min-h-screen md:flex">
  <Sidebar />

  <div class="flex-1 min-w-0">
    <!-- Phone top bar: brand only; page titles live in the page -->
    <header class="md:hidden sticky top-0 z-30 pt-safe bg-gray-50/90 dark:bg-gray-900/90 backdrop-blur border-b border-gray-200/70 dark:border-gray-800">
      <div class="h-12 px-4 flex items-center">
        <a href="#/" aria-label={m.nav_overview()}><Logo /></a>
      </div>
    </header>

    <main id="main" tabindex="-1" class="mx-auto w-full max-w-[1180px] px-4 sm:px-6 lg:px-10 pt-5 sm:pt-8 pb-28 md:pb-12 outline-none">
      {@render children()}
    </main>
  </div>
</div>

<BottomTabs />
<RunLogHost />

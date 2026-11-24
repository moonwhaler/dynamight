<script lang="ts">
  // Old addresses (bookmarks, links in notifications) and their new place
  import { replace, location } from 'svelte-spa-router';

  const MOVED: Record<string, string> = {
    '/about': '/settings/about',
    '/history': '/activity',
  };

  $effect(() => {
    replace(MOVED[$location] ?? '/');
  });
</script>

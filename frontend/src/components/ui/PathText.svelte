<script lang="ts">
  import { shortenPath } from '$lib/utils/path';
  // A path that keeps its end visible: "/mnt/…/photos/2026". Shortened from
  // the middle so the folder names that tell paths apart stay readable.
  let { path, max = 42, class: className = '' }: { path: string; max?: number; class?: string } = $props();

</script>

<span class="font-mono text-[0.92em] {className}" title={path}>{shortenPath(path, max)}</span>

<script lang="ts">
  // The one log viewer of the app; any page opens it with showRunLog().
  import RunLogModal from './RunLogModal.svelte';
  import { openRun, closeRunLog } from '$lib/stores/runLog';
</script>

{#if $openRun}
  {#key $openRun.runId}
    <RunLogModal runId={$openRun.runId} jobId={$openRun.jobId} onClose={closeRunLog} />
  {/key}
{/if}

<script lang="ts">
  import HelpTooltip from '../ui/HelpTooltip.svelte';

  let {
    syncDeletes = $bindable(false),
    checksumMode = $bindable(false),
    compress = $bindable(false),
    dryRun = $bindable(false),
    bandwidthLimit = $bindable<number | null>(null),
    excludes = $bindable<string[]>([]),
    verbosity = $bindable<'quiet' | 'normal' | 'verbose'>('normal'),
  }: {
    syncDeletes: boolean;
    checksumMode: boolean;
    compress: boolean;
    dryRun: boolean;
    bandwidthLimit: number | null;
    excludes: string[];
    verbosity: 'quiet' | 'normal' | 'verbose';
  } = $props();

  let newExclude = $state('');

  function addExclude() {
    const pattern = newExclude.trim();
    if (pattern && !excludes.includes(pattern)) {
      excludes = [...excludes, pattern];
      newExclude = '';
    }
  }

  function removeExclude(pattern: string) {
    excludes = excludes.filter((e) => e !== pattern);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addExclude();
    }
  }
</script>

<div class="space-y-4">
  <h2 class="text-lg font-semibold text-gray-900">Rsync Options</h2>

  <!-- Delete Mode -->
  <div class="flex items-start gap-3">
    <input
      type="checkbox"
      bind:checked={syncDeletes}
      id="syncDeletes"
      class="mt-1 rounded text-primary-600"
    />
    <div>
      <label for="syncDeletes" class="font-medium text-gray-700">
        Mirror Mode (--delete)
        <HelpTooltip text="Creates an exact mirror of the source. If you delete a file from your source, it will also be deleted from the backup on the next run. This keeps your backup clean but means accidentally deleted files won't be recoverable from the backup." />
      </label>
      <p class="text-sm text-gray-500">
        Delete files from destination that no longer exist in source.
        <span class="text-amber-600 font-medium">Use with caution!</span>
      </p>
    </div>
  </div>

  <!-- Checksum Mode -->
  <div class="flex items-start gap-3">
    <input
      type="checkbox"
      bind:checked={checksumMode}
      id="checksumMode"
      class="mt-1 rounded text-primary-600"
    />
    <div>
      <label for="checksumMode" class="font-medium text-gray-700">
        Checksum Mode (--checksum)
        <HelpTooltip text="Normally rsync checks if files changed by comparing size and modification time (fast). Checksum mode reads the entire file content to calculate a hash (slower but catches every change). Useful if file timestamps are unreliable or you need 100% verification." />
      </label>
      <p class="text-sm text-gray-500">
        Compare files by checksum instead of modification time and size. Slower but more accurate.
      </p>
    </div>
  </div>

  <!-- Compression -->
  <div class="flex items-start gap-3">
    <input
      type="checkbox"
      bind:checked={compress}
      id="compress"
      class="mt-1 rounded text-primary-600"
    />
    <div>
      <label for="compress" class="font-medium text-gray-700">
        Compression (-z)
        <HelpTooltip text="Compresses data before sending it over the wire. Great for network backups over slow connections, but not needed for local USB drives. Already-compressed files (videos, images, archives) won't benefit much." />
      </label>
      <p class="text-sm text-gray-500">
        Compress data during transfer. Useful for slow connections, but adds CPU overhead.
      </p>
    </div>
  </div>

  <!-- Dry Run -->
  <div class="flex items-start gap-3">
    <input
      type="checkbox"
      bind:checked={dryRun}
      id="dryRun"
      class="mt-1 rounded text-primary-600"
    />
    <div>
      <label for="dryRun" class="font-medium text-gray-700">
        Dry Run (--dry-run)
        <HelpTooltip text="Simulates the backup without actually copying any files. The logs will show exactly what would happen. Perfect for testing a new job configuration or checking what would be deleted with Mirror Mode enabled." />
      </label>
      <p class="text-sm text-gray-500">
        Show what would be transferred without actually doing it. Good for testing.
      </p>
    </div>
  </div>

  <!-- Verbosity -->
  <div>
    <label for="verbosity" class="block font-medium text-gray-700">
      Output Verbosity
      <HelpTooltip text="Controls how much information rsync outputs during backup. Quiet mode only shows errors. Normal shows files transferred and summary statistics. Verbose adds per-file progress bars and transfer speeds." />
    </label>
    <select id="verbosity" bind:value={verbosity} class="input mt-1 w-48">
      <option value="quiet">Quiet (errors only)</option>
      <option value="normal">Normal (files + stats)</option>
      <option value="verbose">Verbose (full progress)</option>
    </select>
    <p class="text-sm text-gray-500 mt-1">
      {#if verbosity === 'quiet'}
        Only errors will be shown in the logs.
      {:else if verbosity === 'normal'}
        Shows which files are transferred and summary statistics.
      {:else}
        Shows per-file progress bars, speeds, and detailed statistics.
      {/if}
    </p>
  </div>

  <!-- Bandwidth Limit -->
  <div>
    <label for="bandwidth" class="block font-medium text-gray-700">
      Bandwidth Limit (KB/s)
      <HelpTooltip text="Limits how fast rsync transfers data. Useful if you're backing up over a network and don't want to saturate your connection. Value is in kilobytes per second (1000 KB/s = ~1 MB/s). Leave empty for maximum speed." />
    </label>
    <input
      type="number"
      id="bandwidth"
      bind:value={bandwidthLimit}
      placeholder="Unlimited"
      min="0"
      class="input mt-1 w-40"
    />
    <p class="text-sm text-gray-500 mt-1">Leave empty for unlimited.</p>
  </div>

  <!-- Excludes -->
  <div>
    <label class="block font-medium text-gray-700">
      Exclude Patterns
      <HelpTooltip text="Files and folders matching these patterns will be skipped. Use wildcards like *.tmp (all .tmp files), node_modules (specific folder), or .* (all hidden files). Patterns are matched against the relative path from the source directory." />
    </label>
    <div class="mt-2 flex gap-2">
      <input
        type="text"
        bind:value={newExclude}
        onkeydown={handleKeydown}
        placeholder="e.g., *.tmp, .cache, node_modules"
        class="input flex-1"
      />
      <button type="button" onclick={addExclude} class="btn btn-secondary"> Add </button>
    </div>

    {#if excludes.length > 0}
      <div class="mt-3 flex flex-wrap gap-2">
        {#each excludes as pattern}
          <span class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm bg-gray-100">
            <code class="text-gray-800">{pattern}</code>
            <button
              type="button"
              onclick={() => removeExclude(pattern)}
              class="text-gray-400 hover:text-gray-600"
            >
              &times;
            </button>
          </span>
        {/each}
      </div>
    {/if}
  </div>
</div>

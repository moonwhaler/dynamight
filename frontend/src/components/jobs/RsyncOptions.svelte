<script lang="ts">
  let {
    syncDeletes = $bindable(false),
    checksumMode = $bindable(false),
    compress = $bindable(false),
    dryRun = $bindable(false),
    bandwidthLimit = $bindable<number | null>(null),
    excludes = $bindable<string[]>([]),
  }: {
    syncDeletes: boolean;
    checksumMode: boolean;
    compress: boolean;
    dryRun: boolean;
    bandwidthLimit: number | null;
    excludes: string[];
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
      <label for="syncDeletes" class="font-medium text-gray-700">Mirror Mode (--delete)</label>
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
      <label for="checksumMode" class="font-medium text-gray-700">Checksum Mode (--checksum)</label>
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
      <label for="compress" class="font-medium text-gray-700">Compression (-z)</label>
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
      <label for="dryRun" class="font-medium text-gray-700">Dry Run (--dry-run)</label>
      <p class="text-sm text-gray-500">
        Show what would be transferred without actually doing it. Good for testing.
      </p>
    </div>
  </div>

  <!-- Bandwidth Limit -->
  <div>
    <label for="bandwidth" class="block font-medium text-gray-700">Bandwidth Limit (KB/s)</label>
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
    <label class="block font-medium text-gray-700">Exclude Patterns</label>
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

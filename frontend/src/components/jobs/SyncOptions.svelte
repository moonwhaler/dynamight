<script lang="ts">
  import type { SyncOptions, ProviderCapabilities, DestinationType } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';

  let {
    options = $bindable<SyncOptions>(),
    destinationType = 'local',
    capabilities = null,
  }: {
    options: SyncOptions;
    destinationType: DestinationType;
    capabilities: ProviderCapabilities | null;
  } = $props();

  let newExclude = $state('');

  // Default capabilities if not provided
  let effectiveCapabilities = $derived(
    capabilities ?? {
      supports_delete: true,
      supports_compression: destinationType === 'local',
      supports_checksum: destinationType === 'local',
      supports_bandwidth_limit: destinationType === 'local' || destinationType === 'sftp',
      supports_exclude_patterns: true,
      supports_incremental: true,
      supports_dry_run: true,
      requires_credentials: destinationType !== 'local',
    }
  );

  // Rsync-specific options from provider_options
  let checksumMode = $derived(
    (options.provider_options?.checksum_mode as boolean) ?? false
  );
  let compress = $derived((options.provider_options?.compress as boolean) ?? false);

  function setProviderOption(key: string, value: boolean) {
    options.provider_options = {
      ...(options.provider_options ?? {}),
      [key]: value,
    };
  }

  function addExclude() {
    const pattern = newExclude.trim();
    if (pattern && !options.exclude_patterns.includes(pattern)) {
      options.exclude_patterns = [...options.exclude_patterns, pattern];
      newExclude = '';
    }
  }

  function removeExclude(pattern: string) {
    options.exclude_patterns = options.exclude_patterns.filter((e) => e !== pattern);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addExclude();
    }
  }
</script>

<div class="space-y-4">
  <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Sync Options</h2>

  <!-- Delete Mode (Mirror) -->
  {#if effectiveCapabilities.supports_delete}
    <label
      class="flex items-start gap-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/50 rounded-xl cursor-pointer hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors"
    >
      <div class="relative flex items-center">
        <input type="checkbox" bind:checked={options.delete_extraneous} class="peer sr-only" />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-red-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          Mirror Mode
          <HelpTooltip
            text="Creates an exact mirror of the source. If you delete a file from your source, it will also be deleted from the backup on the next run."
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Delete files from destination that no longer exist in source.
          <span class="text-amber-600 dark:text-amber-500 font-medium">Use with caution!</span>
        </p>
      </div>
    </label>
  {/if}

  <!-- Rsync-specific: Checksum Mode -->
  {#if effectiveCapabilities.supports_checksum && destinationType === 'local'}
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input
          type="checkbox"
          checked={checksumMode}
          onchange={(e) => setProviderOption('checksum_mode', e.currentTarget.checked)}
          class="peer sr-only"
        />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          Checksum Mode
          <HelpTooltip
            text="Compare files by checksum instead of modification time and size. Slower but catches every change."
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Compare files by checksum instead of modification time and size. Slower but more accurate.
        </p>
      </div>
    </label>
  {/if}

  <!-- Rsync-specific: Compression -->
  {#if effectiveCapabilities.supports_compression && destinationType === 'local'}
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input
          type="checkbox"
          checked={compress}
          onchange={(e) => setProviderOption('compress', e.currentTarget.checked)}
          class="peer sr-only"
        />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          Compression
          <HelpTooltip
            text="Compress data during transfer. Useful for slow network connections, but adds CPU overhead."
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Compress data during transfer. Useful for slow connections, but adds CPU overhead.
        </p>
      </div>
    </label>
  {/if}

  <!-- Dry Run -->
  {#if effectiveCapabilities.supports_dry_run}
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input type="checkbox" bind:checked={options.dry_run} class="peer sr-only" />
        <div
          class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"
        ></div>
        <div
          class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
        ></div>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
          Dry Run
          <HelpTooltip
            text="Simulates the backup without actually copying any files. Perfect for testing a new job configuration."
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          Show what would be transferred without actually doing it. Good for testing.
        </p>
      </div>
    </label>
  {/if}

  <!-- Verbosity -->
  <div>
    <label for="verbosity" class="block font-medium text-gray-700 dark:text-gray-300">
      Output Verbosity
      <HelpTooltip
        text="Controls how much information is output during backup. Quiet mode only shows errors."
      />
    </label>
    <select id="verbosity" bind:value={options.verbosity} class="input mt-1 w-64">
      <option value="quiet">Quiet (errors only)</option>
      <option value="normal">Normal (files + stats)</option>
      <option value="verbose">Verbose (full progress)</option>
    </select>
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if options.verbosity === 'quiet'}
        Only errors will be shown in the logs.
      {:else if options.verbosity === 'normal'}
        Shows which files are transferred and summary statistics.
      {:else}
        Shows per-file progress bars, speeds, and detailed statistics.
      {/if}
    </p>
  </div>

  <!-- Bandwidth Limit -->
  {#if effectiveCapabilities.supports_bandwidth_limit}
    <div>
      <label for="bandwidth" class="block font-medium text-gray-700 dark:text-gray-300">
        Bandwidth Limit (KB/s)
        <HelpTooltip
          text="Limits how fast data is transferred. Useful if backing up over a network and don't want to saturate the connection."
        />
      </label>
      <input
        type="number"
        id="bandwidth"
        bind:value={options.bandwidth_limit_kbps}
        placeholder="Unlimited"
        min="0"
        class="input mt-1 w-40"
      />
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">Leave empty for unlimited.</p>
    </div>
  {/if}

  <!-- Excludes -->
  {#if effectiveCapabilities.supports_exclude_patterns}
    <div>
      <label class="block font-medium text-gray-700 dark:text-gray-300">
        Exclude Patterns
        <HelpTooltip
          text="Files and folders matching these patterns will be skipped. Use wildcards like *.tmp or specific folder names."
        />
      </label>
      <div class="mt-2 flex gap-2">
        <input
          type="text"
          bind:value={newExclude}
          onkeydown={handleKeydown}
          placeholder="e.g., *.tmp, .cache, node_modules"
          class="input flex-1"
        />
        <button type="button" onclick={addExclude} class="btn btn-secondary">Add</button>
      </div>

      {#if options.exclude_patterns.length > 0}
        <div class="mt-3 flex flex-wrap gap-2">
          {#each options.exclude_patterns as pattern}
            <span
              class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm bg-gray-100 dark:bg-gray-700"
            >
              <code class="text-gray-800 dark:text-gray-200">{pattern}</code>
              <button
                type="button"
                onclick={() => removeExclude(pattern)}
                class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                &times;
              </button>
            </span>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

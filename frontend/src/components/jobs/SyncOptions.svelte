<script lang="ts">
  import type { SyncOptions, ProviderCapabilities, DestinationType } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

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
  let ignoreTimes = $derived((options.provider_options?.ignore_times as boolean) ?? false);

  function setProviderOption(key: string, value: boolean) {
    const newOptions = {
      ...(options.provider_options ?? {}),
      [key]: value,
    };
    // When enabling ignore_times, disable checksum_mode (they're mutually exclusive)
    if (key === 'ignore_times' && value) {
      newOptions.checksum_mode = false;
    }
    options.provider_options = newOptions;
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
  <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{m.sync_options_title()}</h2>

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
          {m.sync_mirror_mode()}
          <HelpTooltip
            text={m.sync_mirror_mode_description()}
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.sync_mirror_desc()}
          <span class="text-amber-600 dark:text-amber-500 font-medium">{m.sync_mirror_warning()}</span>
        </p>
      </div>
    </label>
  {/if}

  <!-- Rsync-specific: Checksum Mode -->
  {#if effectiveCapabilities.supports_checksum && destinationType === 'local'}
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl transition-colors {ignoreTimes ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70'}"
    >
      <div class="relative flex items-center">
        <input
          type="checkbox"
          checked={checksumMode && !ignoreTimes}
          onchange={(e) => setProviderOption('checksum_mode', e.currentTarget.checked)}
          disabled={ignoreTimes}
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
          {m.sync_checksum_title()}
          <HelpTooltip
            text={m.sync_checksum_help()}
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {#if ignoreTimes}
            {m.sync_checksum_disabled_by_force()}
          {:else}
            {m.sync_checksum_desc()}
          {/if}
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
          {m.sync_compression_title()}
          <HelpTooltip
            text={m.sync_compression_help()}
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.sync_compression_desc()}
        </p>
      </div>
    </label>
  {/if}

  <!-- Rsync-specific: Force Sync (Ignore Times) -->
  {#if destinationType === 'local'}
    <label
      class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
    >
      <div class="relative flex items-center">
        <input
          type="checkbox"
          checked={ignoreTimes}
          onchange={(e) => setProviderOption('ignore_times', e.currentTarget.checked)}
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
          {m.sync_ignore_times_title()}
          <HelpTooltip
            text={m.sync_ignore_times_help()}
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.sync_ignore_times_desc()}
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
          {m.sync_dry_run_title()}
          <HelpTooltip
            text={m.sync_dry_run_help()}
          />
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          {m.sync_dry_run_desc()}
        </p>
      </div>
    </label>
  {/if}

  <!-- Space Check Mode (local only) -->
  {#if destinationType === 'local'}
    <div>
      <label for="space_check" class="block font-medium text-gray-700 dark:text-gray-300">
        {m.space_check_title()}
        <HelpTooltip text={m.space_check_help()} />
      </label>
      <select
        id="space_check"
        bind:value={options.space_check}
        class="input mt-1 w-64"
      >
        <option value="warn">{m.space_check_warn()}</option>
        <option value="fail">{m.space_check_fail()}</option>
        <option value="none">{m.space_check_none()}</option>
      </select>
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
        {#if options.space_check === 'fail'}
          {m.space_check_fail_desc()}
        {:else if options.space_check === 'none'}
          {m.space_check_none_desc()}
        {:else}
          {m.space_check_warn_desc()}
        {/if}
      </p>
    </div>
  {/if}

  <!-- Verbosity -->
  <div>
    <label for="verbosity" class="block font-medium text-gray-700 dark:text-gray-300">
      {m.sync_verbosity_title()}
      <HelpTooltip
        text={m.sync_verbosity_help()}
      />
    </label>
    <select id="verbosity" bind:value={options.verbosity} class="input mt-1 w-64">
      <option value="quiet">{m.sync_verbosity_quiet_option()}</option>
      <option value="normal">{m.sync_verbosity_normal_option()}</option>
      <option value="verbose">{m.sync_verbosity_verbose_option()}</option>
    </select>
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if options.verbosity === 'quiet'}
        {m.sync_verbosity_quiet_desc()}
      {:else if options.verbosity === 'normal'}
        {m.sync_verbosity_normal_desc()}
      {:else}
        {m.sync_verbosity_verbose_desc()}
      {/if}
    </p>
  </div>

  <!-- Bandwidth Limit -->
  {#if effectiveCapabilities.supports_bandwidth_limit}
    <div>
      <label for="bandwidth" class="block font-medium text-gray-700 dark:text-gray-300">
        {m.sync_bandwidth_title()}
        <HelpTooltip
          text={m.sync_bandwidth_help()}
        />
      </label>
      <input
        type="number"
        id="bandwidth"
        bind:value={options.bandwidth_limit_kbps}
        placeholder={m.sync_bandwidth_unlimited()}
        min="0"
        class="input mt-1 w-40"
      />
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.sync_bandwidth_leave_empty()}</p>
    </div>
  {/if}

  <!-- Excludes -->
  {#if effectiveCapabilities.supports_exclude_patterns}
    <div>
      <label class="block font-medium text-gray-700 dark:text-gray-300">
        {m.sync_exclude_title()}
        <HelpTooltip
          text={m.sync_exclude_help()}
        />
      </label>
      <div class="mt-2 flex gap-2">
        <input
          type="text"
          bind:value={newExclude}
          onkeydown={handleKeydown}
          placeholder={m.sync_exclude_placeholder()}
          class="input flex-1"
        />
        <button type="button" onclick={addExclude} class="btn btn-secondary">{m.common_add()}</button>
      </div>

      {#if options.exclude_patterns.length > 0}
        <div class="mt-3 flex flex-wrap gap-2">
          {#each options.exclude_patterns as pattern}
            <span
              class="inline-flex items-center gap-1.5 pl-2.5 pr-1.5 py-1 rounded-full text-sm bg-gray-100 dark:bg-gray-700"
            >
              <code class="text-gray-800 dark:text-gray-200">{pattern}</code>
              <button
                type="button"
                onclick={() => removeExclude(pattern)}
                class="p-0.5 rounded-full text-gray-400 hover:text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors"
                aria-label={m.common_delete()}
              >
                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </span>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

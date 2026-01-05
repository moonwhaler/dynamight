<script lang="ts">
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

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
  <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{m.sync_options_title()}</h2>

  <!-- Delete Mode -->
  <label class="flex items-start gap-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/50 rounded-xl cursor-pointer hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors">
    <div class="relative flex items-center">
      <input type="checkbox" bind:checked={syncDeletes} class="peer sr-only" />
      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-red-600 transition-colors"></div>
      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.sync_mirror_mode()} (--delete)
        <HelpTooltip text={m.sync_mirror_mode_description()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {m.sync_mirror_desc()}
        <span class="text-amber-600 dark:text-amber-500 font-medium">{m.sync_mirror_warning()}</span>
      </p>
    </div>
  </label>

  <!-- Checksum Mode -->
  <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
    <div class="relative flex items-center">
      <input type="checkbox" bind:checked={checksumMode} class="peer sr-only" />
      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.sync_checksum_title()} (--checksum)
        <HelpTooltip text={m.sync_checksum_help()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {m.sync_checksum_desc()}
      </p>
    </div>
  </label>

  <!-- Compression -->
  <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
    <div class="relative flex items-center">
      <input type="checkbox" bind:checked={compress} class="peer sr-only" />
      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.sync_compression_title()} (-z)
        <HelpTooltip text={m.sync_compression_help()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {m.sync_compression_desc()}
      </p>
    </div>
  </label>

  <!-- Dry Run -->
  <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
    <div class="relative flex items-center">
      <input type="checkbox" bind:checked={dryRun} class="peer sr-only" />
      <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
      <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.sync_dry_run_title()} (--dry-run)
        <HelpTooltip text={m.sync_dry_run_help()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {m.sync_dry_run_desc()}
      </p>
    </div>
  </label>

  <!-- Verbosity -->
  <div>
    <label for="verbosity" class="block font-medium text-gray-700 dark:text-gray-300">
      {m.sync_verbosity_title()}
      <HelpTooltip text={m.sync_verbosity_help()} />
    </label>
    <select id="verbosity" bind:value={verbosity} class="input mt-1 w-64">
      <option value="quiet">{m.sync_verbosity_quiet_option()}</option>
      <option value="normal">{m.sync_verbosity_normal_option()}</option>
      <option value="verbose">{m.sync_verbosity_verbose_option()}</option>
    </select>
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {#if verbosity === 'quiet'}
        {m.sync_verbosity_quiet_desc()}
      {:else if verbosity === 'normal'}
        {m.sync_verbosity_normal_desc()}
      {:else}
        {m.sync_verbosity_verbose_desc()}
      {/if}
    </p>
  </div>

  <!-- Bandwidth Limit -->
  <div>
    <label for="bandwidth" class="block font-medium text-gray-700 dark:text-gray-300">
      {m.sync_bandwidth_title()}
      <HelpTooltip text={m.sync_bandwidth_help()} />
    </label>
    <input
      type="number"
      id="bandwidth"
      bind:value={bandwidthLimit}
      placeholder={m.sync_bandwidth_unlimited()}
      min="0"
      class="input mt-1 w-40"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.sync_bandwidth_leave_empty()}</p>
  </div>

  <!-- Excludes -->
  <div>
    <label class="block font-medium text-gray-700 dark:text-gray-300">
      {m.sync_exclude_title()}
      <HelpTooltip text={m.sync_exclude_help()} />
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

    {#if excludes.length > 0}
      <div class="mt-3 flex flex-wrap gap-2">
        {#each excludes as pattern}
          <span class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-sm bg-gray-100 dark:bg-gray-700">
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
</div>

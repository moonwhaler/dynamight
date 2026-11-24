<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import type { SyncOptions, ProviderCapabilities, DestinationType, DirectoryEntry } from '../../lib/types';
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { api } from '../../lib/api';

  let {
    options = $bindable<SyncOptions>(),
    destinationType = 'local',
    capabilities = null,
    sourceDirs = [],
    part = 'all',
  }: {
    options: SyncOptions;
    destinationType: DestinationType;
    capabilities: ProviderCapabilities | null;
    sourceDirs: string[];
    /** Which group of options to show: the editor places them in different sections */
    part?: 'all' | 'basic' | 'advanced' | 'excludes';
  } = $props();

  const show = (p: 'basic' | 'advanced' | 'excludes') => part === 'all' || part === p;

  let newExclude = $state('');

  // True when the archive-compression feature is enabled for this job.
  // Used to show a contextual note in the Excluded Directories section.
  // Note: `compress` (line below) is a separate rsync wire-compression option.
  let compressEnabled = $derived(options.compress_dirs?.enabled ?? false);

  // Selected source directory for exclude browsing
  let selectedExcludeSource = $state('');

  // Exclude directories browser state
  let showExcludeBrowser = $state(false);
  let excludeBrowserSourceDir = $state('');
  let excludeCurrentPath = $state('');
  let excludeEntries = $state<DirectoryEntry[]>([]);
  let excludeLoading = $state(false);
  let selectedExcludes = $state<Set<string>>(new Set());

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

  // Exclude directories functions
  async function openExcludeBrowser(sourceDir: string) {
    excludeBrowserSourceDir = sourceDir;
    excludeCurrentPath = sourceDir;
    selectedExcludes = new Set();
    showExcludeBrowser = true;
    await browseExcludePath(sourceDir);
  }

  async function browseExcludePath(path: string) {
    excludeLoading = true;
    try {
      const result = await api.system.browse(path);
      excludeCurrentPath = result.path;
      // Only show directories that are children of the source dir
      excludeEntries = result.entries.filter(e => e.is_dir);
    } catch {
      excludeEntries = [];
    } finally {
      excludeLoading = false;
    }
  }

  function closeExcludeBrowser() {
    showExcludeBrowser = false;
    selectedExcludes = new Set();
  }

  function toggleExcludeSelection(path: string) {
    const newSet = new Set(selectedExcludes);
    if (newSet.has(path)) {
      newSet.delete(path);
    } else {
      newSet.add(path);
    }
    selectedExcludes = newSet;
  }

  function addSelectedExcludes() {
    const newDirs = [...selectedExcludes].filter(p => !options.exclude_dirs.includes(p));
    if (newDirs.length > 0) {
      options.exclude_dirs = [...options.exclude_dirs, ...newDirs];
    }
    closeExcludeBrowser();
  }

  function removeExcludeDir(path: string) {
    options.exclude_dirs = options.exclude_dirs.filter(p => p !== path);
  }

  // Get parent source directory for an excluded path
  function getParentSource(excludePath: string): string | undefined {
    return sourceDirs.find(source => excludePath.startsWith(source + '/'));
  }

  // Get relative path from source for display
  function getRelativePath(excludePath: string): string {
    const parent = getParentSource(excludePath);
    if (parent) {
      return excludePath.slice(parent.length + 1);
    }
    return excludePath;
  }

  // Get folder name from source path for button display
  function getSourceFolderName(sourcePath: string): string {
    return sourcePath.split('/').filter(Boolean).pop() || sourcePath;
  }

  // Check if a path is already excluded
  function isAlreadyExcluded(path: string): boolean {
    return options.exclude_dirs.includes(path);
  }

  // Get entries that can be selected (not already excluded)
  let selectableExcludeEntries = $derived(
    excludeEntries.filter(e => !isAlreadyExcluded(e.path))
  );

  // Group excluded dirs by their parent source directory (ordered by sourceDirs)
  let excludesBySource = $derived.by(() => {
    const result: Array<{ sourceDir: string; paths: string[] }> = [];
    for (const src of sourceDirs) {
      const paths = options.exclude_dirs.filter(ex => ex.startsWith(src + '/'));
      if (paths.length > 0) {
        result.push({ sourceDir: src, paths });
      }
    }
    return result;
  });

  // Paths that don't match any current source directory
  let orphanedExcludes = $derived(
    options.exclude_dirs.filter(ex => !sourceDirs.some(src => ex.startsWith(src + '/')))
  );

  // Select all visible directories
  function selectAllExcludes() {
    const newSet = new Set(selectedExcludes);
    selectableExcludeEntries.forEach(e => newSet.add(e.path));
    selectedExcludes = newSet;
  }

  // Clear selection
  function clearExcludeSelection() {
    selectedExcludes = new Set();
  }

  // Navigate to parent in exclude browser
  async function navigateToParent() {
    const parent = excludeCurrentPath.split('/').slice(0, -1).join('/') || '/';
    // Don't navigate above the source dir
    if (excludeCurrentPath !== excludeBrowserSourceDir && parent.startsWith(excludeBrowserSourceDir.split('/').slice(0, -1).join('/'))) {
      await browseExcludePath(parent);
    }
  }

  // Cleanup invalid exclude_dirs when sourceDirs change
  $effect(() => {
    if (options.exclude_dirs && options.exclude_dirs.length > 0 && sourceDirs.length > 0) {
      const validExcludes = options.exclude_dirs.filter(ex =>
        sourceDirs.some(src => ex.startsWith(src + '/') && ex !== src)
      );
      if (validExcludes.length !== options.exclude_dirs.length) {
        options.exclude_dirs = validExcludes;
      }
    }
  });

  // Set default selected source when sourceDirs change
  $effect(() => {
    if (sourceDirs.length > 0 && (!selectedExcludeSource || !sourceDirs.includes(selectedExcludeSource))) {
      selectedExcludeSource = sourceDirs[0];
    } else if (sourceDirs.length === 0) {
      selectedExcludeSource = '';
    }
  });
</script>

<div class="space-y-6">
  {#if show('basic')}
    <!-- Mirror mode: deletes in the backup what was deleted at the source -->
    {#if effectiveCapabilities.supports_delete}
      <div class="space-y-2">
        <Toggle label={m.sync_mirror_mode()} description={m.sync_mirror_desc()} bind:checked={options.delete_extraneous}>
          {#snippet extra()}<HelpTooltip text={m.sync_mirror_mode_description()} />{/snippet}
        </Toggle>
        {#if options.delete_extraneous}
          <p class="flex items-start gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-300" role="note">
            {m.sync_mirror_warning_long()}
          </p>
        {/if}
      </div>
    {/if}

    {#if destinationType === 'local'}
      <div>
        <label for="space_check" class="label">
          {m.space_check_title()}
          <HelpTooltip text={m.space_check_help()} />
        </label>
        <select id="space_check" bind:value={options.space_check} class="input sm:w-72">
          <option value="warn">{m.space_check_warn()}</option>
          <option value="fail">{m.space_check_fail()}</option>
          <option value="none">{m.space_check_none()}</option>
        </select>
        <p class="hint">
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
  {/if}

  {#if show('advanced')}
    {#if effectiveCapabilities.supports_dry_run}
      <Toggle label={m.sync_dry_run_title()} description={m.sync_dry_run_desc()} bind:checked={options.dry_run}>
        {#snippet extra()}<HelpTooltip text={m.sync_dry_run_help()} />{/snippet}
      </Toggle>
    {/if}

    {#if effectiveCapabilities.supports_checksum && destinationType === 'local'}
      <Toggle
        label={m.sync_checksum_title()}
        description={ignoreTimes ? m.sync_checksum_disabled_by_force() : m.sync_checksum_desc()}
        checked={checksumMode && !ignoreTimes}
        disabled={ignoreTimes}
        onchange={(v) => setProviderOption('checksum_mode', v)}
      >
        {#snippet extra()}<HelpTooltip text={m.sync_checksum_help()} />{/snippet}
      </Toggle>
    {/if}

    {#if destinationType === 'local'}
      <Toggle
        label={m.sync_ignore_times_title()}
        description={m.sync_ignore_times_desc()}
        checked={ignoreTimes}
        onchange={(v) => setProviderOption('ignore_times', v)}
      >
        {#snippet extra()}<HelpTooltip text={m.sync_ignore_times_help()} />{/snippet}
      </Toggle>
    {/if}

    {#if effectiveCapabilities.supports_compression && destinationType === 'local'}
      <Toggle
        label={m.sync_compression_title()}
        description={m.sync_compression_desc()}
        checked={compress}
        onchange={(v) => setProviderOption('compress', v)}
      >
        {#snippet extra()}<HelpTooltip text={m.sync_compression_help()} />{/snippet}
      </Toggle>
    {/if}

    <div class="grid gap-6 sm:grid-cols-2">
      <div>
        <label for="verbosity" class="label">
          {m.sync_verbosity_title()}
          <HelpTooltip text={m.sync_verbosity_help()} />
        </label>
        <select id="verbosity" bind:value={options.verbosity} class="input">
          <option value="quiet">{m.sync_verbosity_quiet_option()}</option>
          <option value="normal">{m.sync_verbosity_normal_option()}</option>
          <option value="verbose">{m.sync_verbosity_verbose_option()}</option>
        </select>
        <p class="hint">
          {#if options.verbosity === 'quiet'}
            {m.sync_verbosity_quiet_desc()}
          {:else if options.verbosity === 'normal'}
            {m.sync_verbosity_normal_desc()}
          {:else}
            {m.sync_verbosity_verbose_desc()}
          {/if}
        </p>
      </div>

      {#if effectiveCapabilities.supports_bandwidth_limit}
        <div>
          <label for="bandwidth" class="label">
            {m.sync_bandwidth_title()}
            <HelpTooltip text={m.sync_bandwidth_help()} />
          </label>
          <input
            type="number"
            id="bandwidth"
            bind:value={options.bandwidth_limit_kbps}
            placeholder={m.sync_bandwidth_unlimited()}
            min="0"
            class="input"
          />
          <p class="hint">{m.sync_bandwidth_leave_empty()}</p>
        </div>
      {/if}
    </div>
  {/if}

  {#if show('excludes')}
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

  <!-- Excluded Directories -->
  {#if effectiveCapabilities.supports_exclude_patterns}
    <div>
      <label class="block font-medium text-gray-700 dark:text-gray-300">
        {m.exclude_dirs_title()}
        <HelpTooltip text={m.exclude_dirs_help()} />
      </label>
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
        {m.exclude_dirs_description()}
      </p>

      {#if compressEnabled}
        <p class="text-xs text-indigo-600 dark:text-indigo-400 mt-1 flex items-center gap-1.5">
          <svg class="w-3.5 h-3.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          {m.exclude_dirs_also_applied_to_compression()}
        </p>
      {/if}

      {#if sourceDirs.length === 0}
        <p class="text-sm text-gray-400 dark:text-gray-500 italic mt-2">
          {m.exclude_dirs_add_sources_first()}
        </p>
      {:else}
        <!-- Currently excluded directories -->
        {#if options.exclude_dirs && options.exclude_dirs.length > 0}
          {#if sourceDirs.length <= 1}
            <!-- Single source: flat list, no header needed -->
            <div class="mt-3 border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden divide-y divide-gray-200 dark:divide-gray-700">
              {#each options.exclude_dirs as path (path)}
                <div class="flex items-center justify-between gap-3 px-3 py-2.5 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-750">
                  <div class="flex items-center gap-2.5 min-w-0">
                    <svg class="w-5 h-5 flex-shrink-0 text-red-400" fill="currentColor" viewBox="0 0 20 20">
                      <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                    </svg>
                    <code class="text-sm text-gray-700 dark:text-gray-200 truncate">{getRelativePath(path)}</code>
                  </div>
                  <button
                    type="button"
                    onclick={() => removeExcludeDir(path)}
                    class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                    aria-label={m.common_delete()}
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <!-- Multi-source: grouped cards per source directory -->
            <div class="mt-3 space-y-2">
              {#each excludesBySource as { sourceDir, paths } (sourceDir)}
                <div
                  class="border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden"
                  role="group"
                  aria-label={getSourceFolderName(sourceDir)}
                >
                  <!-- Group header -->
                  <div class="flex items-center gap-2 px-3 py-2 bg-gray-50 dark:bg-gray-800/70 border-b border-gray-200 dark:border-gray-700">
                    <svg class="w-4 h-4 flex-shrink-0 text-yellow-500" fill="currentColor" viewBox="0 0 20 20">
                      <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                    </svg>
                    <div class="min-w-0 flex-1 flex items-baseline gap-2">
                      <span class="text-xs font-semibold text-gray-700 dark:text-gray-200 flex-shrink-0">{getSourceFolderName(sourceDir)}</span>
                      <code class="text-xs text-gray-400 dark:text-gray-500 truncate">{sourceDir}</code>
                    </div>
                    <span class="text-xs text-gray-400 dark:text-gray-500 flex-shrink-0 tabular-nums">{paths.length}</span>
                  </div>
                  <!-- Excluded paths in this group -->
                  <div class="divide-y divide-gray-200 dark:divide-gray-700">
                    {#each paths as path (path)}
                      <div class="flex items-center justify-between gap-3 px-3 py-2.5 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-750">
                        <div class="flex items-center gap-2.5 min-w-0">
                          <svg class="w-5 h-5 flex-shrink-0 text-red-400" fill="currentColor" viewBox="0 0 20 20">
                            <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                          </svg>
                          <code class="text-sm text-gray-700 dark:text-gray-200 truncate">{getRelativePath(path)}</code>
                        </div>
                        <button
                          type="button"
                          onclick={() => removeExcludeDir(path)}
                          class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                          aria-label={m.common_delete()}
                        >
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              {/each}

              <!-- Orphaned exclusions: paths that no longer match any source directory -->
              {#if orphanedExcludes.length > 0}
                <div
                  class="border border-amber-200 dark:border-amber-800/50 rounded-xl overflow-hidden"
                  role="group"
                  aria-label={m.exclude_dirs_orphaned()}
                >
                  <div class="flex items-center gap-2 px-3 py-2 bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800/50">
                    <svg class="w-4 h-4 flex-shrink-0 text-amber-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                    </svg>
                    <span class="text-xs font-medium text-amber-700 dark:text-amber-300">{m.exclude_dirs_orphaned()}</span>
                  </div>
                  <div class="divide-y divide-gray-200 dark:divide-gray-700">
                    {#each orphanedExcludes as path (path)}
                      <div class="flex items-center justify-between gap-3 px-3 py-2.5 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-750">
                        <div class="flex items-center gap-2.5 min-w-0">
                          <svg class="w-5 h-5 flex-shrink-0 text-amber-400" fill="currentColor" viewBox="0 0 20 20">
                            <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                          </svg>
                          <code class="text-sm text-gray-700 dark:text-gray-200 truncate">{path}</code>
                        </div>
                        <button
                          type="button"
                          onclick={() => removeExcludeDir(path)}
                          class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                          aria-label={m.common_delete()}
                        >
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        {/if}

        <!-- Source directory selector + Browse button -->
        <div class="mt-3 flex flex-col sm:flex-row gap-2">
          {#if sourceDirs.length > 1}
            <select
              bind:value={selectedExcludeSource}
              class="input flex-1"
            >
              {#each sourceDirs as sourceDir}
                <option value={sourceDir}>{getSourceFolderName(sourceDir)}</option>
              {/each}
            </select>
          {/if}
          <button
            type="button"
            onclick={() => openExcludeBrowser(selectedExcludeSource)}
            disabled={!selectedExcludeSource}
            class="btn btn-secondary"
            title={selectedExcludeSource}
          >
            <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            {m.exclude_dirs_browse()}
          </button>
        </div>
      {/if}
    </div>
  {/if}
  {/if}
</div>

<!-- Exclude Directory Browser Modal -->
<Dialog
  bind:open={() => showExcludeBrowser, (v) => { if (!v) closeExcludeBrowser(); }}
  title={m.exclude_dirs_select()}
  size="lg"
  closeOnBackdrop={false}
  bodyClass="p-0 flex flex-col h-[70dvh] sm:h-[34rem]"
>
  {#snippet header()}
    <span class="hidden sm:block max-w-[16rem] truncate self-center font-mono text-xs text-gray-500 dark:text-gray-400" title={excludeCurrentPath}>{excludeCurrentPath}</span>
  {/snippet}
      <!-- Navigation & Quick Actions -->
      <div class="px-2 sm:px-4 py-2 sm:py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-900/30 flex-shrink-0">
        <div class="flex flex-wrap gap-1.5 sm:gap-2 items-center">
          <button
            type="button"
            onclick={navigateToParent}
            disabled={excludeCurrentPath === excludeBrowserSourceDir}
            class="btn btn-secondary text-xs sm:text-sm py-1.5 sm:py-2 px-2 sm:px-3 inline-flex items-center"
            title={m.path_selector_up()}
          >
            <svg class="w-4 h-4 sm:mr-1.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 11l5-5m0 0l5 5m-5-5v12" />
            </svg>
            <span class="hidden sm:inline">{m.path_selector_up()}</span>
          </button>
          <div class="flex-1"></div>
          {#if selectableExcludeEntries.length > 0}
            <button type="button" onclick={selectAllExcludes} class="text-xs sm:text-sm text-primary-600 dark:text-primary-400 hover:underline font-medium px-1.5 sm:px-2">
              <span class="hidden xs:inline">{m.path_selector_select_all()}</span>
              <span class="xs:hidden">{m.common_all()}</span>
            </button>
          {/if}
          {#if selectedExcludes.size > 0}
            <button type="button" onclick={clearExcludeSelection} class="text-xs sm:text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 px-1.5 sm:px-2">
              <span class="hidden xs:inline">{m.path_selector_clear_selection({ count: selectedExcludes.size })}</span>
              <span class="xs:hidden">Clear ({selectedExcludes.size})</span>
            </button>
          {/if}
        </div>
      </div>

      <!-- Directory Listing -->
      <div class="flex-1 min-h-0 overflow-y-auto p-2 sm:p-3">
        {#if excludeLoading}
          <div class="flex items-center justify-center h-full">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if excludeEntries.length === 0}
          <div class="flex flex-col items-center justify-center h-full text-gray-500 dark:text-gray-400">
            <svg class="w-10 h-10 sm:w-12 sm:h-12 mb-2 sm:mb-3 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <p class="text-sm sm:text-base">{m.exclude_dirs_no_subdirectories()}</p>
          </div>
        {:else}
          <div class="space-y-0.5">
            {#each excludeEntries as entry}
              {@const alreadyExcluded = isAlreadyExcluded(entry.path)}
              {@const isSelected = selectedExcludes.has(entry.path)}
              <div
                class="flex items-center gap-2 sm:gap-3 px-2 sm:px-3 py-2.5 rounded-xl transition-colors {alreadyExcluded ? 'bg-green-50 dark:bg-green-900/20' : isSelected ? 'bg-primary-50 dark:bg-primary-900/30' : 'hover:bg-gray-100 dark:hover:bg-gray-700/50'} active:bg-gray-200 dark:active:bg-gray-700"
              >
                <!-- Checkbox for selection -->
                {#if alreadyExcluded}
                  <div class="w-5 h-5 flex items-center justify-center flex-shrink-0" title={m.exclude_dirs_already_excluded()}>
                    <svg class="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                      <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                    </svg>
                  </div>
                {:else}
                  <button
                    type="button"
                    onclick={() => toggleExcludeSelection(entry.path)}
                    class="w-5 h-5 rounded border-2 flex items-center justify-center transition-colors flex-shrink-0 {isSelected ? 'bg-primary-600 border-primary-600' : 'border-gray-300 dark:border-gray-600 hover:border-primary-500'}"
                    aria-label={isSelected ? 'Deselect directory' : 'Select directory'}
                  >
                    {#if isSelected}
                      <svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                      </svg>
                    {/if}
                  </button>
                {/if}

                <!-- Folder icon and name (clickable to navigate) -->
                <button
                  type="button"
                  onclick={() => browseExcludePath(entry.path)}
                  class="flex-1 flex items-center gap-2 sm:gap-2.5 text-left min-w-0"
                >
                  <svg class="w-5 h-5 flex-shrink-0 {alreadyExcluded ? 'text-green-500' : 'text-yellow-500'}" fill="currentColor" viewBox="0 0 20 20">
                    <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                  </svg>
                  <span class="text-sm sm:text-base truncate {alreadyExcluded ? 'text-green-700 dark:text-green-400' : 'text-gray-900 dark:text-gray-100'}">{entry.name}</span>
                </button>

                <!-- Quick add button for individual directory - hidden on mobile -->
                {#if !alreadyExcluded && !isSelected}
                  <button
                    type="button"
                    onclick={() => toggleExcludeSelection(entry.path)}
                    class="hidden sm:block text-xs font-medium text-primary-600 dark:text-primary-400 hover:text-primary-700 dark:hover:text-primary-300 flex-shrink-0 px-2 py-1 rounded-lg hover:bg-primary-50 dark:hover:bg-primary-900/30 transition-colors"
                    title={m.path_selector_add_to_selection()}
                  >
                    +{m.common_add()}
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

  {#snippet footer()}
    <p class="text-sm text-gray-600 dark:text-gray-400 self-center text-center sm:text-left sm:mr-auto">
      {#if selectedExcludes.size > 0}
        {selectedExcludes.size === 1 ? m.exclude_dirs_one_selected() : m.exclude_dirs_n_selected({ count: selectedExcludes.size })}
      {:else}
        {m.exclude_dirs_select_to_add()}
      {/if}
    </p>
    <button type="button" onclick={closeExcludeBrowser} class="btn btn-secondary">{m.common_cancel()}</button>
    <button type="button" onclick={addSelectedExcludes} disabled={selectedExcludes.size === 0} class="btn btn-primary">
      {#if selectedExcludes.size > 0}
        {selectedExcludes.size === 1 ? m.exclude_dirs_add_one() : m.exclude_dirs_add_n({ count: selectedExcludes.size })}
      {:else}
        {m.path_selector_add_selected()}
      {/if}
    </button>
  {/snippet}
</Dialog>

<script lang="ts">
  import type { SyncOptions, DestinationType, CompressDirsOptions } from '../../lib/types';
  import SinglePathSelector from './SinglePathSelector.svelte';
  import HelpTooltip from '../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    options = $bindable<SyncOptions>(),
    sourceDirs = [],
    destinationType = 'local',
  }: {
    options: SyncOptions;
    sourceDirs: string[];
    destinationType: DestinationType;
  } = $props();

  let compressEnabled = $derived(options.compress_dirs?.enabled ?? false);
  let isCloudProvider = $derived(destinationType !== 'local');
  // Number of excluded directories currently configured (non-optional field, no ?.)
  let excludedDirCount = $derived(options.exclude_dirs.length);
  let showMirrorWarning = $derived(
    compressEnabled &&
    (options.compress_dirs?.add_timestamp ?? false) &&
    (options.compress_dirs?.max_archives_per_dir ?? null) != null &&
    options.delete_extraneous
  );

  let showPassword = $state(false);

  function toggleCompression(checked: boolean) {
    if (checked) {
      if (options.compress_dirs == null) {
        options.compress_dirs = {
          enabled: true,
          format: 'tar_gz',
          store_only: false,
          add_timestamp: true,
          custom_name: null,
          max_archives_per_dir: null,
          staging_path: '',
          password: null,
        };
      } else {
        options.compress_dirs.enabled = true;
      }
    } else {
      if (options.compress_dirs != null) {
        options.compress_dirs.enabled = false;
      }
    }
  }

  function setTimestamp(checked: boolean) {
    if (options.compress_dirs != null) {
      options.compress_dirs.add_timestamp = checked;
      if (!checked) {
        // max_archives is only meaningful with timestamps
        options.compress_dirs.max_archives_per_dir = null;
      }
    }
  }

  function setMaxArchives(raw: string) {
    if (options.compress_dirs != null) {
      const val = raw === '' ? null : parseInt(raw, 10);
      options.compress_dirs.max_archives_per_dir =
        val != null && !isNaN(val) && val >= 1 ? val : null;
    }
  }

  function setCustomName(raw: string) {
    if (options.compress_dirs != null) {
      options.compress_dirs.custom_name = raw.trim() === '' ? null : raw;
    }
  }

  function setPassword(raw: string) {
    if (options.compress_dirs != null) {
      options.compress_dirs.password = raw === '' ? null : raw;
    }
  }

  function setStoreOnly(checked: boolean) {
    if (options.compress_dirs != null) {
      options.compress_dirs.store_only = checked;
    }
  }
</script>

<div class="space-y-3">
  <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
    {m.compress_dirs_title()}
  </h2>

  <!-- Master toggle -->
  <label
    class="flex items-start gap-4 p-4 bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800/50 rounded-xl cursor-pointer hover:bg-indigo-100 dark:hover:bg-indigo-900/30 transition-colors"
  >
    <div class="relative flex items-center mt-0.5">
      <input
        type="checkbox"
        checked={compressEnabled}
        onchange={(e) => toggleCompression(e.currentTarget.checked)}
        class="peer sr-only"
      />
      <div
        class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-indigo-600 transition-colors"
      ></div>
      <div
        class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
      ></div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
        {m.compress_dirs_title()}
        <HelpTooltip text={m.compress_dirs_help()} />
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
        {m.compress_dirs_description()}
      </p>
    </div>
  </label>

  <!-- Options panel (shown when enabled) -->
  {#if compressEnabled && options.compress_dirs != null}
    {@const cd = options.compress_dirs}
    <div
      class="bg-white dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700 rounded-xl p-4 space-y-5"
    >

      <!-- Excluded directories carry-over notice -->
      {#if excludedDirCount > 0}
        <div class="flex items-center gap-2 p-3 bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800/50 rounded-xl text-xs text-indigo-700 dark:text-indigo-300">
          <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span>
            {excludedDirCount === 1
              ? m.compress_dirs_excluded_dirs_applied_one()
              : m.compress_dirs_excluded_dirs_applied_other({ count: excludedDirCount })}
          </span>
        </div>
      {/if}

      <!-- 1. Staging Path -->
      <div>
        <label class="block font-medium text-gray-700 dark:text-gray-300 text-sm mb-1">
          {m.compress_dirs_staging_path()}
          <HelpTooltip text={m.compress_dirs_staging_path_help()} />
        </label>
        <SinglePathSelector
          bind:path={options.compress_dirs.staging_path}
          placeholder={m.compress_dirs_staging_path_placeholder()}
        />
        {#if cd.staging_path === ''}
          <p class="mt-1.5 text-xs text-amber-600 dark:text-amber-400 font-medium">
            {m.compress_dirs_staging_path_required()}
          </p>
        {/if}
      </div>

      <!-- 2. Archive Format -->
      <div>
        <label for="compress-format" class="block font-medium text-gray-700 dark:text-gray-300 text-sm mb-1">
          {m.compress_dirs_format()}
          <HelpTooltip text={m.compress_dirs_format_help()} />
        </label>
        <select
          id="compress-format"
          bind:value={options.compress_dirs.format}
          class="input w-full sm:w-auto"
        >
          <option value="tar_gz">{m.compress_dirs_format_targz()}</option>
          <option value="zip">{m.compress_dirs_format_zip()}</option>
        </select>
      </div>

      <!-- 2b. Store only toggle -->
      <label
        class="flex items-start gap-4 p-3 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
      >
        <div class="relative flex items-center mt-0.5">
          <input
            type="checkbox"
            checked={cd.store_only ?? false}
            onchange={(e) => setStoreOnly(e.currentTarget.checked)}
            class="peer sr-only"
          />
          <div
            class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-indigo-600 transition-colors"
          ></div>
          <div
            class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
          ></div>
        </div>
        <div class="flex-1 min-w-0">
          <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
            {m.compress_dirs_store_only()}
            <HelpTooltip text={m.compress_dirs_store_only_help()} />
          </div>
        </div>
      </label>

      <!-- 3. Password -->
      <div>
        <label for="compress-password" class="block font-medium text-gray-700 dark:text-gray-300 text-sm mb-1">
          {m.compress_dirs_password()}
          <HelpTooltip text={m.compress_dirs_password_help()} />
        </label>
        <div class="relative w-full sm:w-64">
          <input
            id="compress-password"
            type={showPassword ? 'text' : 'password'}
            value={cd.password ?? ''}
            oninput={(e) => setPassword(e.currentTarget.value)}
            placeholder={m.compress_dirs_password_placeholder()}
            class="input w-full pr-10"
            autocomplete="new-password"
          />
          <button
            type="button"
            onclick={() => (showPassword = !showPassword)}
            class="absolute inset-y-0 right-0 flex items-center px-3 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            aria-label={showPassword ? 'Hide password' : 'Show password'}
          >
            {#if showPassword}
              <!-- eye-off icon -->
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
              </svg>
            {:else}
              <!-- eye icon -->
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
            {/if}
          </button>
        </div>
        {#if cd.password && cd.format !== 'zip'}
          <p class="mt-1 text-xs text-gray-400 dark:text-gray-500">
            {m.compress_dirs_password_enc_note()}
          </p>
        {/if}
      </div>

      <!-- 4. Add Timestamp toggle -->
      <div class="space-y-3">
        <label
          class="flex items-start gap-4 p-3 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors"
        >
          <div class="relative flex items-center mt-0.5">
            <input
              type="checkbox"
              checked={cd.add_timestamp}
              onchange={(e) => setTimestamp(e.currentTarget.checked)}
              class="peer sr-only"
            />
            <div
              class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-indigo-600 transition-colors"
            ></div>
            <div
              class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"
            ></div>
          </div>
          <div class="flex-1 min-w-0">
            <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
              {m.compress_dirs_timestamp()}
              <HelpTooltip text={m.compress_dirs_timestamp_help()} />
            </div>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
              {m.compress_dirs_timestamp_example()}
            </p>
          </div>
        </label>

        <!-- Max local archives (only shown when timestamp is enabled) -->
        {#if cd.add_timestamp}
          <div class="pl-4 border-l-2 border-indigo-200 dark:border-indigo-800 space-y-2">
            <label for="compress-max-archives" class="block font-medium text-gray-700 dark:text-gray-300 text-sm">
              {m.compress_dirs_max_archives()}
              <HelpTooltip text={m.compress_dirs_max_archives_help()} />
            </label>
            <input
              id="compress-max-archives"
              type="number"
              min="1"
              value={cd.max_archives_per_dir ?? ''}
              oninput={(e) => setMaxArchives(e.currentTarget.value)}
              placeholder={m.compress_dirs_max_archives_placeholder()}
              class="input w-36"
            />

            {#if showMirrorWarning}
              <div class="flex items-start gap-2 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800/50 rounded-xl text-xs text-amber-800 dark:text-amber-300">
                <svg class="w-4 h-4 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                <span>{m.compress_dirs_mirror_warning()}</span>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- 5. Custom Name Prefix (optional) -->
      <div>
        <label for="compress-custom-name" class="block font-medium text-gray-700 dark:text-gray-300 text-sm mb-1">
          {m.compress_dirs_custom_name()}
          <HelpTooltip text={m.compress_dirs_custom_name_help()} />
        </label>
        <input
          id="compress-custom-name"
          type="text"
          value={cd.custom_name ?? ''}
          oninput={(e) => setCustomName(e.currentTarget.value)}
          placeholder={m.compress_dirs_custom_name_placeholder()}
          maxlength="64"
          class="input w-full sm:w-64"
        />
        <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
          {m.compress_dirs_per_source_note()}
        </p>
      </div>

      <!-- Cloud provider note -->
      {#if isCloudProvider}
        <div class="flex items-start gap-2 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800/50 rounded-xl text-xs text-blue-800 dark:text-blue-300">
          <svg class="w-4 h-4 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span>{m.compress_dirs_cloud_warning()}</span>
        </div>
      {/if}
    </div>

  {:else if !compressEnabled && options.compress_dirs != null}
    <!-- Collapsed summary when disabled but previously configured -->
    <p class="text-xs text-gray-400 dark:text-gray-500 pl-1">
      {m.compress_dirs_disabled_summary()}
    </p>
  {/if}
</div>

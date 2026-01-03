<script lang="ts">
  import { api } from '../../lib/api';
  import type { DirectoryEntry } from '../../lib/types';

  let { paths = $bindable<string[]>([]) }: { paths: string[] } = $props();

  let showBrowser = $state(false);
  let currentPath = $state('/');
  let entries = $state<DirectoryEntry[]>([]);
  let loading = $state(false);
  let manualPath = $state('');

  async function browse(path: string) {
    loading = true;
    try {
      const result = await api.system.browse(path);
      currentPath = result.path;
      entries = result.entries;
    } catch {
      entries = [];
    } finally {
      loading = false;
    }
  }

  function openBrowser() {
    showBrowser = true;
    browse('/');
  }

  function closeBrowser() {
    showBrowser = false;
  }

  function selectPath(path: string) {
    if (!paths.includes(path)) {
      paths = [...paths, path];
    }
    closeBrowser();
  }

  function removePath(path: string) {
    paths = paths.filter((p) => p !== path);
  }

  function addManualPath() {
    const path = manualPath.trim();
    if (path && !paths.includes(path)) {
      paths = [...paths, path];
      manualPath = '';
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addManualPath();
    }
  }

  function goUp() {
    const parent = currentPath.split('/').slice(0, -1).join('/') || '/';
    browse(parent);
  }
</script>

<div class="space-y-3">
  <!-- Selected Paths -->
  {#if paths.length > 0}
    <div class="space-y-2">
      {#each paths as path}
        <div class="flex items-center justify-between p-2 bg-gray-50 rounded-lg">
          <code class="text-sm text-gray-800">{path}</code>
          <button onclick={() => removePath(path)} class="text-red-500 hover:text-red-700" aria-label="Remove path">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Add Path -->
  <div class="flex gap-2">
    <input
      type="text"
      bind:value={manualPath}
      onkeydown={handleKeydown}
      placeholder="Enter path manually or browse..."
      class="input flex-1"
    />
    <button type="button" onclick={addManualPath} class="btn btn-secondary">Add</button>
    <button type="button" onclick={openBrowser} class="btn btn-secondary">Browse</button>
  </div>
</div>

<!-- File Browser Modal -->
{#if showBrowser}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    onclick={(e) => e.target === e.currentTarget && closeBrowser()}
    onkeydown={(e) => e.key === 'Escape' && closeBrowser()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="bg-white rounded-xl shadow-xl max-w-2xl w-full max-h-[80vh] flex flex-col">
      <div class="p-4 border-b flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900">Browse Filesystem</h3>
          <p class="text-sm text-gray-500 font-mono">{currentPath}</p>
        </div>
        <button onclick={closeBrowser} class="text-gray-400 hover:text-gray-600" aria-label="Close browser">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      <div class="p-2 border-b flex gap-2">
        <button onclick={goUp} disabled={currentPath === '/'} class="btn btn-secondary text-sm">
          Up
        </button>
        <button onclick={() => selectPath(currentPath)} class="btn btn-primary text-sm">
          Select Current Directory
        </button>
      </div>

      <div class="flex-1 overflow-auto p-2">
        {#if loading}
          <div class="flex justify-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else}
          <div class="space-y-1">
            {#each entries as entry}
              {#if entry.is_dir}
                <button
                  onclick={() => browse(entry.path)}
                  class="w-full flex items-center gap-2 p-2 hover:bg-gray-100 rounded text-left"
                >
                  <svg class="w-5 h-5 text-yellow-500" fill="currentColor" viewBox="0 0 20 20">
                    <path
                      d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"
                    />
                  </svg>
                  <span class="text-gray-900">{entry.name}</span>
                </button>
              {:else}
                <div class="flex items-center gap-2 p-2 text-gray-400">
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                    />
                  </svg>
                  <span>{entry.name}</span>
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

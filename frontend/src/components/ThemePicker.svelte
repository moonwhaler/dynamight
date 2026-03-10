<script lang="ts">
  import { themeStore } from '../lib/stores/theme';
  import { bases, accents, type BaseTheme, type AccentColor } from '../lib/theme-presets';

  let open = $state(false);
  let panelRef = $state<HTMLDivElement | null>(null);

  function handleClickOutside(event: MouseEvent) {
    if (panelRef && !panelRef.contains(event.target as Node)) {
      open = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') open = false;
  }

  $effect(() => {
    if (open) {
      document.addEventListener('click', handleClickOutside);
      document.addEventListener('keydown', handleKeydown);
    }
    return () => {
      document.removeEventListener('click', handleClickOutside);
      document.removeEventListener('keydown', handleKeydown);
    };
  });

  const baseKeys = Object.keys(bases) as BaseTheme[];
  const accentKeys = Object.keys(accents) as AccentColor[];
</script>

<div class="relative" bind:this={panelRef}>
  <!-- Trigger button -->
  <button
    onclick={() => open = !open}
    class="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 rounded-lg transition-colors"
    title="Theme"
    aria-label="Theme"
    aria-expanded={open}
  >
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.8">
      <path stroke-linecap="round" stroke-linejoin="round" d="M4.098 19.902a3.75 3.75 0 005.304 0l6.401-6.402M6.75 21A3.75 3.75 0 013 17.25V4.125C3 3.504 3.504 3 4.125 3h5.25c.621 0 1.125.504 1.125 1.125v4.072M6.75 21a3.75 3.75 0 003.75-3.75V8.197M6.75 21h13.125c.621 0 1.125-.504 1.125-1.125v-5.25c0-.621-.504-1.125-1.125-1.125h-4.072M10.5 8.197l2.88-2.88c.438-.439 1.15-.439 1.59 0l3.712 3.713c.44.44.44 1.152 0 1.59l-2.879 2.88M6.75 17.25h.008v.008H6.75v-.008z" />
    </svg>
  </button>

  <!-- Panel -->
  {#if open}
    <div
      class="absolute right-0 mt-2 w-64 origin-top-right rounded-2xl bg-white dark:bg-gray-800 shadow-xl ring-1 ring-gray-200 dark:ring-gray-700 z-50 overflow-hidden"
      role="dialog"
      aria-label="Theme picker"
    >
      <div class="p-4 space-y-5">

        <!-- Mode toggle -->
        <div class="flex items-center justify-between">
          <span class="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">Mode</span>
          <div class="flex bg-gray-100 dark:bg-gray-700 rounded-lg p-0.5">
            <button
              onclick={() => themeStore.setMode('light')}
              class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all {$themeStore.mode === 'light'
                ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
              </svg>
              Light
            </button>
            <button
              onclick={() => themeStore.setMode('dark')}
              class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all {$themeStore.mode === 'dark'
                ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'}"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
              </svg>
              Dark
            </button>
          </div>
        </div>

        <!-- Divider -->
        <div class="border-t border-gray-100 dark:border-gray-700"></div>

        <!-- Base tone -->
        <div>
          <span class="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">Style</span>
          <div class="flex gap-2 mt-2.5">
            {#each baseKeys as key}
              {@const base = bases[key]}
              {@const isActive = $themeStore.base === key}
              <button
                onclick={() => themeStore.setBase(key)}
                class="group flex-1 flex flex-col items-center gap-1.5 p-2 rounded-xl transition-all {isActive
                  ? 'bg-gray-100 dark:bg-gray-700 ring-2 ring-primary-500'
                  : 'hover:bg-gray-50 dark:hover:bg-gray-700/50'}"
                title={base.label}
              >
                <div
                  class="w-8 h-8 rounded-full border-2 transition-all flex items-center justify-center {isActive
                    ? 'border-primary-500 scale-110'
                    : 'border-gray-200 dark:border-gray-600 group-hover:border-gray-300 dark:group-hover:border-gray-500'}"
                  style="background-color: {base.swatch}"
                >
                  {#if isActive}
                    <svg class="w-3.5 h-3.5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="3">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  {/if}
                </div>
                <span class="text-[10px] font-medium {isActive ? 'text-gray-900 dark:text-white' : 'text-gray-500 dark:text-gray-400'}">{base.label}</span>
              </button>
            {/each}
          </div>
        </div>

        <!-- Divider -->
        <div class="border-t border-gray-100 dark:border-gray-700"></div>

        <!-- Accent color -->
        <div>
          <span class="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">Accent</span>
          <div class="grid grid-cols-6 gap-2 mt-2.5">
            {#each accentKeys as key}
              {@const accent = accents[key]}
              {@const isActive = $themeStore.accent === key}
              <button
                onclick={() => themeStore.setAccent(key)}
                class="group flex flex-col items-center gap-1.5"
                title={accent.label}
              >
                <div
                  class="w-7 h-7 rounded-full transition-all flex items-center justify-center {isActive
                    ? 'ring-2 ring-offset-2 ring-offset-white dark:ring-offset-gray-800 ring-gray-400 dark:ring-gray-500 scale-110'
                    : 'hover:scale-105'}"
                  style="background-color: {accent.swatch}"
                >
                  {#if isActive}
                    <svg class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="3">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  {/if}
                </div>
                <span class="text-[9px] font-medium {isActive ? 'text-gray-900 dark:text-white' : 'text-gray-400 dark:text-gray-500'}">{accent.label}</span>
              </button>
            {/each}
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

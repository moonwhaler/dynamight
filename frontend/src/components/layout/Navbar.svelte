<script lang="ts">
  import { authStore } from '../../lib/stores/auth';
  import { themeStore } from '../../lib/stores/theme';
  import SettingsModal from '../SettingsModal.svelte';
  import BackupRestoreModal from '../BackupRestoreModal.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { onMenuToggle }: { onMenuToggle?: () => void } = $props();

  let showSettingsModal = $state(false);
  let showBackupModal = $state(false);
  let showSettingsMenu = $state(false);
  let menuRef = $state<HTMLDivElement | null>(null);
  let logoLoaded = $state(false);
  let logoError = $state(false);

  async function handleLogout() {
    await authStore.logout();
  }

  function handleClickOutside(event: MouseEvent) {
    if (menuRef && !menuRef.contains(event.target as Node)) {
      showSettingsMenu = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      showSettingsMenu = false;
    }
  }

  $effect(() => {
    if (showSettingsMenu) {
      document.addEventListener('click', handleClickOutside);
      document.addEventListener('keydown', handleKeydown);
    }
    return () => {
      document.removeEventListener('click', handleClickOutside);
      document.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<nav class="sticky top-0 z-40 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-4 sm:px-6 py-3">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <!-- Mobile hamburger menu -->
      <button
        onclick={onMenuToggle}
        class="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 rounded-lg md:hidden"
        aria-label="Open menu"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>

      <!-- Custom logo (if logo.png exists in public folder) -->
      {#if !logoError}
        <img
          src="/logo.png"
          alt={m.auth_app_name()}
          class="h-8 w-auto object-contain"
          class:hidden={!logoLoaded}
          onload={() => logoLoaded = true}
          onerror={() => logoError = true}
        />
      {/if}

      <!-- Fallback: icon + text (shown while loading or if logo doesn't exist) -->
      {#if !logoLoaded}
        <div class="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
          <svg
            class="w-5 h-5 text-white"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
            />
          </svg>
        </div>
        <h1 class="text-xl font-bold text-gray-900 dark:text-white">{m.auth_app_name()}</h1>
      {/if}
    </div>

    <div class="flex items-center gap-2 sm:gap-3">
      <span class="hidden sm:inline text-sm text-gray-600 dark:text-gray-300">
        {$authStore.user?.username}
      </span>

      <!-- Settings Dropdown -->
      <div class="relative" bind:this={menuRef}>
        <button
          onclick={() => showSettingsMenu = !showSettingsMenu}
          class="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 dark:text-gray-400 dark:hover:text-gray-200 dark:hover:bg-gray-700 rounded-lg transition-colors"
          title={m.common_settings()}
          aria-label={m.common_settings()}
          aria-expanded={showSettingsMenu}
          aria-haspopup="true"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>

        {#if showSettingsMenu}
          <div
            class="absolute right-0 mt-2 w-48 sm:w-56 origin-top-right rounded-xl bg-white dark:bg-gray-800 shadow-lg ring-1 ring-black/5 dark:ring-white/10 focus:outline-none z-50 overflow-hidden"
            role="menu"
            aria-orientation="vertical"
          >
            <div class="py-1">
              <!-- Username (mobile only) -->
              <div class="sm:hidden px-4 py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm font-medium text-gray-900 dark:text-white">{$authStore.user?.username}</span>
              </div>

              <!-- Theme Toggle -->
              <button
                onclick={() => { themeStore.toggle(); showSettingsMenu = false; }}
                class="w-full flex items-center gap-3 px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
                role="menuitem"
              >
                {#if $themeStore === 'dark'}
                  <svg class="w-5 h-5 text-amber-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
                  </svg>
                  <span>{m.settings_light_mode()}</span>
                {:else}
                  <svg class="w-5 h-5 text-indigo-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
                  </svg>
                  <span>{m.settings_dark_mode()}</span>
                {/if}
              </button>

              <div class="border-t border-gray-100 dark:border-gray-700 my-1"></div>

              <!-- Settings -->
              <button
                onclick={() => { showSettingsModal = true; showSettingsMenu = false; }}
                class="w-full flex items-center gap-3 px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
                role="menuitem"
              >
                <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                <span>{m.common_settings()}</span>
              </button>

              <!-- Backup & Restore -->
              <button
                onclick={() => { showBackupModal = true; showSettingsMenu = false; }}
                class="w-full flex items-center gap-3 px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
                role="menuitem"
              >
                <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
                </svg>
                <span>{m.backup_menu_label()}</span>
              </button>

              <!-- Mobile Logout -->
              <div class="sm:hidden border-t border-gray-100 dark:border-gray-700 my-1"></div>
              <button
                onclick={handleLogout}
                class="sm:hidden w-full flex items-center gap-3 px-4 py-2.5 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
                role="menuitem"
              >
                <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                </svg>
                <span>{m.auth_logout()}</span>
              </button>
            </div>
          </div>
        {/if}
      </div>

      <button
        onclick={handleLogout}
        class="hidden sm:inline-flex btn btn-secondary text-sm"
      >
        {m.auth_logout()}
      </button>
    </div>
  </div>
</nav>

<SettingsModal bind:open={showSettingsModal} />
<BackupRestoreModal bind:open={showBackupModal} />

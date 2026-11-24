<script lang="ts">
  // Desktop: full sidebar (lg+). Tablet / foldable: icon rail (md–lg).
  import { location } from 'svelte-spa-router';
  import { authStore } from '$lib/stores/auth';
  import { themeStore } from '$lib/stores/theme';
  import Icon from '../ui/Icon.svelte';
  import Menu from '../ui/Menu.svelte';
  import Logo from './Logo.svelte';
  import { NAV_ITEMS, isActive } from './nav';
  import * as m from '$lib/paraglide/messages.js';

  const username = $derived($authStore.user?.username ?? '');
</script>

<aside
  class="hidden md:flex flex-col sticky top-0 h-screen shrink-0 w-[5.5rem] lg:w-60 border-r border-gray-200 dark:border-gray-800 bg-white/60 dark:bg-gray-900"
  aria-label={m.nav_main()}
>
  <div class="h-16 flex items-center justify-center lg:justify-start lg:px-5">
    <a href="#/" class="hidden lg:block" aria-label={m.nav_overview()}><Logo /></a>
    <a href="#/" class="lg:hidden" aria-label={m.nav_overview()}><Logo compact /></a>
  </div>

  <nav class="flex-1 px-2 lg:px-3 pt-2">
    <ul class="space-y-1">
      {#each NAV_ITEMS as item (item.path)}
        {@const active = isActive(item.path, $location)}
        <li>
          <a
            href="#{item.path}"
            aria-current={active ? 'page' : undefined}
            class="group flex flex-col lg:flex-row items-center gap-1 lg:gap-3 px-1 lg:px-3 py-2 lg:py-2 rounded-lg transition-colors
              {active
              ? 'text-primary-700 dark:text-primary-300'
              : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100'}
              {active ? 'lg:bg-primary-50 lg:dark:bg-primary-900/30' : 'lg:hover:bg-gray-100 lg:dark:hover:bg-gray-800'}"
          >
            <span
              class="flex items-center justify-center w-12 h-8 lg:w-auto lg:h-auto rounded-full transition-colors
                {active ? 'bg-primary-100 dark:bg-primary-900/50 lg:bg-transparent lg:dark:bg-transparent' : 'group-hover:bg-gray-100 dark:group-hover:bg-gray-800 lg:group-hover:bg-transparent lg:dark:group-hover:bg-transparent'}"
            >
              <Icon name={item.icon} class="w-[22px] h-[22px] lg:w-5 lg:h-5" />
            </span>
            <span class="text-[11px] lg:text-sm font-medium leading-tight text-center">{item.label()}</span>
          </a>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="p-2 lg:p-3 border-t border-gray-200 dark:border-gray-800">
    <div class="flex flex-col lg:flex-row items-center gap-2">
      <span
        class="w-9 h-9 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-200 flex items-center justify-center text-sm font-semibold uppercase shrink-0"
        title={username}
        aria-hidden="true">{username.slice(0, 1)}</span
      >
      <span class="hidden lg:block flex-1 min-w-0 text-sm font-medium text-gray-800 dark:text-gray-200 truncate">{username}</span>
      <Menu
        label={m.nav_account_menu()}
        icon="more"
        triggerClass="btn btn-ghost btn-icon btn-sm"
        align="start"
        items={[
          {
            label: $themeStore.mode === 'dark' ? m.theme_light() : m.theme_dark(),
            icon: $themeStore.mode === 'dark' ? 'sun' : 'moon',
            onclick: () => themeStore.toggle(),
          },
          { label: m.theme_title(), icon: 'swatch', href: '#/settings/appearance' },
          { label: m.settings_tab_account(), icon: 'user', href: '#/settings/account' },
          'separator',
          { label: m.auth_logout(), icon: 'logout', onclick: () => authStore.logout() },
        ]}
      />
    </div>
  </div>
</aside>

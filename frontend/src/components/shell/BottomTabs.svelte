<script lang="ts">
  // Phone navigation: five tabs at the bottom, within thumb reach.
  import { location } from 'svelte-spa-router';
  import Icon from '../ui/Icon.svelte';
  import { NAV_ITEMS, isActive } from './nav';
  import * as m from '$lib/paraglide/messages.js';
</script>

<nav
  class="md:hidden fixed bottom-0 inset-x-0 z-40 border-t border-gray-200 dark:border-gray-800 bg-white/95 dark:bg-gray-900/95 backdrop-blur pb-safe"
  aria-label={m.nav_main()}
>
  <ul class="grid grid-cols-5">
    {#each NAV_ITEMS as item (item.path)}
      {@const active = isActive(item.path, $location)}
      <li>
        <a
          href="#{item.path}"
          aria-current={active ? 'page' : undefined}
          class="flex flex-col items-center gap-0.5 pt-2 pb-1.5 {active ? 'text-primary-700 dark:text-primary-300' : 'text-gray-500 dark:text-gray-400'}"
        >
          <span class="flex items-center justify-center w-14 h-7 rounded-full transition-colors {active ? 'bg-primary-100 dark:bg-primary-900/50' : ''}">
            <Icon name={item.icon} class="w-[22px] h-[22px]" />
          </span>
          <span class="text-[11px] font-medium">{item.label()}</span>
        </a>
      </li>
    {/each}
  </ul>
</nav>

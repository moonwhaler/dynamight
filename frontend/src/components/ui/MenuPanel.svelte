<script lang="ts">
  import Icon from './Icon.svelte';
  import type { MenuItem } from './Menu.svelte';

  let {
    id,
    label,
    items,
    style,
    onChoose,
    onClose,
  }: {
    id: string;
    label: string;
    items: MenuItem[];
    /** Position (top/left/right/bottom) of the fixed panel */
    style: string;
    onChoose: (item: Exclude<MenuItem, 'separator'>) => void;
    /** Escape (refocus the trigger) or Tab (leave focus alone) */
    onClose: (refocus: boolean) => void;
  } = $props();

  let panel = $state<HTMLDivElement | null>(null);

  function itemEls(): HTMLElement[] {
    return panel ? Array.from(panel.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')) : [];
  }

  export function focusItem(i: number) {
    const els = itemEls();
    if (els.length) els[(i + els.length) % els.length].focus();
  }

  export function contains(node: Node | null): boolean {
    return !!node && !!panel?.contains(node);
  }

  function onKey(e: KeyboardEvent) {
    const els = itemEls();
    const cur = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusItem(cur + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusItem(cur - 1);
    } else if (e.key === 'Home') {
      e.preventDefault();
      focusItem(0);
    } else if (e.key === 'End') {
      e.preventDefault();
      focusItem(els.length - 1);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose(true);
    } else if (e.key === 'Tab') {
      onClose(false);
    }
  }
</script>

<div
  bind:this={panel}
  {id}
  role="menu"
  tabindex="-1"
  aria-label={label}
  onkeydown={onKey}
  class="fixed z-[60] min-w-[12rem] max-w-[18rem] py-1.5 rounded-xl bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 shadow-xl"
  {style}
>
  {#each items as item, i (i)}
    {#if item === 'separator'}
      <div class="my-1.5 border-t border-gray-200 dark:border-gray-700" role="separator"></div>
    {:else if item.href}
      <a
        href={item.href}
        role="menuitem"
        tabindex="-1"
        onclick={() => onClose(false)}
        class="flex items-center gap-3 px-3.5 py-2 text-sm text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700/60 focus:bg-gray-100 dark:focus:bg-gray-700/60 outline-none"
      >
        {#if item.icon}<Icon name={item.icon} class="w-4 h-4 text-gray-400" />{/if}
        {item.label}
      </a>
    {:else}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        disabled={item.disabled}
        onclick={() => onChoose(item)}
        class="w-full flex items-center gap-3 px-3.5 py-2 text-sm text-left outline-none disabled:opacity-50 disabled:cursor-not-allowed
          {item.danger
          ? 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/30 focus:bg-red-50 dark:focus:bg-red-900/30'
          : 'text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700/60 focus:bg-gray-100 dark:focus:bg-gray-700/60'}"
      >
        {#if item.icon}<Icon name={item.icon} class="w-4 h-4 {item.danger ? '' : 'text-gray-400'}" />{/if}
        {item.label}
      </button>
    {/if}
  {/each}
</div>

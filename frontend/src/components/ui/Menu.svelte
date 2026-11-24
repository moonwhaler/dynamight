<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export type MenuItem =
    | {
        label: string;
        icon?: IconName;
        onclick?: () => void;
        href?: string;
        danger?: boolean;
        disabled?: boolean;
      }
    | 'separator';
</script>

<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    label,
    items,
    icon = 'more',
    trigger,
    triggerClass = 'btn btn-ghost btn-icon',
    align = 'end',
  }: {
    /** Accessible name of the trigger button */
    label: string;
    items: MenuItem[];
    icon?: IconName;
    /** Custom trigger content (the button itself is provided) */
    trigger?: Snippet;
    triggerClass?: string;
    align?: 'start' | 'end';
  } = $props();

  let open = $state(false);
  let button = $state<HTMLButtonElement | null>(null);
  let panel = $state<HTMLDivElement | null>(null);
  let pos = $state({ top: 0, left: 0, right: 0, up: false });

  const id = `menu-${Math.random().toString(36).slice(2, 9)}`;

  function place() {
    if (!button) return;
    const r = button.getBoundingClientRect();
    const up = r.bottom + 280 > window.innerHeight && r.top > 280;
    pos = { top: up ? r.top : r.bottom, left: r.left, right: window.innerWidth - r.right, up };
  }

  async function show() {
    place();
    open = true;
    await tick();
    focusItem(0);
  }

  function hide(refocus = true) {
    open = false;
    if (refocus) button?.focus();
  }

  function itemsEls(): HTMLElement[] {
    return panel ? Array.from(panel.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')) : [];
  }

  function focusItem(i: number) {
    const els = itemsEls();
    if (els.length) els[(i + els.length) % els.length].focus();
  }

  function onKey(e: KeyboardEvent) {
    const els = itemsEls();
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
      hide();
    } else if (e.key === 'Tab') {
      hide(false);
    }
  }

  function choose(item: Exclude<MenuItem, 'separator'>) {
    hide();
    item.onclick?.();
  }

  $effect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => {
      if (!panel?.contains(e.target as Node) && !button?.contains(e.target as Node)) hide(false);
    };
    const close = () => hide(false);
    document.addEventListener('mousedown', outside);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('mousedown', outside);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
    };
  });
</script>

<button
  bind:this={button}
  type="button"
  class={triggerClass}
  aria-label={label}
  title={trigger ? undefined : label}
  aria-haspopup="menu"
  aria-expanded={open}
  aria-controls={open ? id : undefined}
  onclick={(e) => {
    e.stopPropagation();
    if (open) hide(false);
    else show();
  }}
  onkeydown={(e) => {
    if (e.key === 'ArrowDown' && !open) {
      e.preventDefault();
      show();
    }
  }}
>
  {#if trigger}{@render trigger()}{:else}<Icon name={icon} />{/if}
</button>

{#if open}
  <div
    bind:this={panel}
    {id}
    role="menu"
    tabindex="-1"
    aria-label={label}
    onkeydown={onKey}
    class="fixed z-[60] min-w-[12rem] max-w-[18rem] py-1.5 rounded-xl bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 shadow-xl"
    style="{pos.up ? `bottom:${window.innerHeight - pos.top + 6}px` : `top:${pos.top + 6}px`};
      {align === 'end' ? `right:${Math.max(8, pos.right)}px` : `left:${Math.max(8, pos.left)}px`}"
  >
    {#each items as item, i (i)}
      {#if item === 'separator'}
        <div class="my-1.5 border-t border-gray-200 dark:border-gray-700" role="separator"></div>
      {:else if item.href}
        <a
          href={item.href}
          role="menuitem"
          tabindex="-1"
          onclick={() => hide(false)}
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
          onclick={() => choose(item)}
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
{/if}

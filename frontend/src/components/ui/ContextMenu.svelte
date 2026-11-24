<script lang="ts">
  import { tick } from 'svelte';
  import MenuPanel from './MenuPanel.svelte';
  import type { MenuItem } from './Menu.svelte';

  /** A menu shown at a point: right-click, long-press or the menu key */
  let {
    open = $bindable(false),
    x,
    y,
    label,
    items,
    returnFocus,
  }: {
    open?: boolean;
    x: number;
    y: number;
    label: string;
    items: MenuItem[];
    /** Element focused again when the menu closes with Escape */
    returnFocus?: HTMLElement | null;
  } = $props();

  let panel = $state<MenuPanel | null>(null);
  const id = `ctx-${Math.random().toString(36).slice(2, 9)}`;

  // Open towards the side with room; 36 px per item is close enough
  const style = $derived.by(() => {
    const height = items.length * 36 + 12;
    const vertical = y + height > window.innerHeight && y > height ? `bottom:${window.innerHeight - y}px` : `top:${y}px`;
    const horizontal = x + 288 > window.innerWidth ? `right:${Math.max(8, window.innerWidth - x)}px` : `left:${x}px`;
    return `${vertical};${horizontal}`;
  });

  function close(refocus = true) {
    open = false;
    if (refocus) returnFocus?.focus();
  }

  function choose(item: Exclude<MenuItem, 'separator'>) {
    close();
    item.onclick?.();
  }

  $effect(() => {
    if (!open) return;
    tick().then(() => panel?.focusItem(0));
    const outside = (e: MouseEvent) => {
      if (!panel?.contains(e.target as Node)) close(false);
    };
    const dismiss = () => close(false);
    document.addEventListener('mousedown', outside);
    window.addEventListener('resize', dismiss);
    window.addEventListener('scroll', dismiss, true);
    return () => {
      document.removeEventListener('mousedown', outside);
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('scroll', dismiss, true);
    };
  });
</script>

{#if open}
  <MenuPanel bind:this={panel} {id} {label} {items} {style} onChoose={choose} onClose={close} />
{/if}

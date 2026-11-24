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
  import MenuPanel from './MenuPanel.svelte';

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
  let panel = $state<MenuPanel | null>(null);
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
    panel?.focusItem(0);
  }

  function hide(refocus = true) {
    open = false;
    if (refocus) button?.focus();
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
  <MenuPanel
    bind:this={panel}
    {id}
    {label}
    {items}
    onChoose={choose}
    onClose={hide}
    style="{pos.up ? `bottom:${window.innerHeight - pos.top + 6}px` : `top:${pos.top + 6}px`};
      {align === 'end' ? `right:${Math.max(8, pos.right)}px` : `left:${Math.max(8, pos.left)}px`}"
  />
{/if}

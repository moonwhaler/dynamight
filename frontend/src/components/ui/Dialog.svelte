<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  type Size = 'sm' | 'md' | 'lg' | 'xl' | 'full';

  let {
    open = $bindable(false),
    title,
    description,
    size = 'md',
    dismissible = true,
    closeOnBackdrop = true,
    canClose,
    onclose,
    header,
    children,
    footer,
    bodyClass = 'px-5 py-4 sm:px-6',
    role = 'dialog',
  }: {
    open?: boolean;
    title: string;
    description?: string;
    size?: Size;
    /** Close on Escape and on a click outside the panel */
    dismissible?: boolean;
    /** Close on a click outside the panel (off for dialogs holding a form) */
    closeOnBackdrop?: boolean;
    /** Asked before closing; return false to stay open (e.g. unsaved input) */
    canClose?: () => boolean | Promise<boolean>;
    onclose?: () => void;
    /** Extra content in the header row, before the close button */
    header?: Snippet;
    children: Snippet;
    footer?: Snippet;
    bodyClass?: string;
    role?: 'dialog' | 'alertdialog';
  } = $props();

  let el = $state<HTMLDialogElement | null>(null);
  const titleId = `dlg-${Math.random().toString(36).slice(2, 9)}`;

  const widths: Record<Size, string> = {
    sm: 'sm:max-w-sm',
    md: 'sm:max-w-lg',
    lg: 'sm:max-w-2xl',
    xl: 'sm:max-w-4xl',
    full: 'sm:max-w-6xl',
  };

  $effect(() => {
    if (!el) return;
    if (open && !el.open) {
      el.showModal?.();
      // Toasts live in the top layer too; let them move above this dialog
      window.dispatchEvent(new CustomEvent('dm:dialog-open'));
    } else if (!open && el.open) {
      el.close?.();
    }
  });

  async function requestClose() {
    if (canClose && !(await canClose())) return;
    open = false;
    onclose?.();
  }

  function handleCancel(e: Event) {
    // Escape: the browser would close the dialog without asking
    e.preventDefault();
    if (dismissible) requestClose();
  }

  function handleClick(e: MouseEvent) {
    if (dismissible && closeOnBackdrop && e.target === el) requestClose();
  }
</script>

<dialog
  bind:this={el}
  {role}
  aria-labelledby={titleId}
  oncancel={handleCancel}
  onclick={handleClick}
  class="dm-dialog p-0 bg-transparent text-inherit max-w-none max-h-none w-full m-0 mt-auto sm:m-auto {widths[size]}
    {size === 'full' ? 'h-[100dvh] sm:h-[min(100dvh-4rem,56rem)]' : ''}"
>
  {#if open}
    <div
      class="flex flex-col bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 shadow-2xl
        rounded-t-2xl sm:rounded-2xl max-h-[92dvh] sm:max-h-[min(100dvh-4rem,56rem)]
        {size === 'full' ? 'h-full max-h-none rounded-none sm:rounded-2xl' : ''}"
    >
      <header class="flex items-start gap-3 px-5 sm:px-6 pt-4 pb-3 border-b border-gray-200 dark:border-gray-700">
        <div class="min-w-0 flex-1 pt-1">
          <h2 id={titleId} class="text-base font-semibold text-gray-900 dark:text-white truncate">{title}</h2>
          {#if description}
            <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">{description}</p>
          {/if}
        </div>
        {#if header}{@render header()}{/if}
        <button type="button" class="btn btn-ghost btn-icon -mr-2" onclick={requestClose} aria-label={m.common_close()}>
          <Icon name="x" />
        </button>
      </header>

      <div class="flex-1 min-h-0 overflow-y-auto overscroll-contain {bodyClass}">
        {@render children()}
      </div>

      {#if footer}
        <footer
          class="flex flex-col-reverse sm:flex-row sm:justify-end gap-2 px-5 sm:px-6 py-3 border-t border-gray-200 dark:border-gray-700 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:pb-3"
        >
          {@render footer()}
        </footer>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  .dm-dialog[open] {
    animation: dm-dialog-in 0.16s ease-out;
  }
  @keyframes dm-dialog-in {
    from {
      opacity: 0;
      transform: translateY(0.75rem);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>

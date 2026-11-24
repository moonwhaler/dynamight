<script lang="ts" module>
  import { writable } from 'svelte/store';

  export interface ConfirmOptions {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    variant?: 'danger' | 'warning' | 'info';
    /** The user must type this text before confirming (for irreversible actions) */
    requireText?: string;
  }

  interface DialogState {
    open: boolean;
    options: ConfirmOptions;
    resolve: ((value: boolean) => void) | null;
  }

  const dialogStore = writable<DialogState>({
    open: false,
    options: { title: '', message: '' },
    resolve: null,
  });

  export function confirm(options: ConfirmOptions): Promise<boolean> {
    return new Promise((resolve) => {
      dialogStore.set({
        open: true,
        options,
        resolve,
      });
    });
  }

  function closeDialog(result: boolean) {
    dialogStore.update((s) => {
      s.resolve?.(result);
      return { open: false, options: { title: '', message: '' }, resolve: null };
    });
  }

  export { dialogStore };
</script>

<script lang="ts">
  import * as m from '$lib/paraglide/messages.js';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  let open = $state(false);
  let options = $state<ConfirmOptions>({ title: '', message: '' });
  let typed = $state('');

  dialogStore.subscribe((s) => {
    open = s.open;
    options = s.options;
    typed = '';
  });

  const variant = $derived(options.variant || 'danger');
  const blocked = $derived(!!options.requireText && typed.trim() !== options.requireText);

  const tone = {
    danger: 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400',
    warning: 'bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400',
    info: 'bg-primary-100 dark:bg-primary-900/30 text-primary-600 dark:text-primary-400',
  };

  const confirmClass = {
    danger: 'btn btn-danger',
    warning: 'btn bg-amber-600 hover:bg-amber-700 text-white',
    info: 'btn btn-primary',
  };
</script>

<Dialog
  bind:open={() => open, (v) => { if (!v) closeDialog(false); }}
  title={options.title}
  size="sm"
  role="alertdialog"
>
  <div class="flex items-start gap-4">
    <div class="shrink-0 w-10 h-10 rounded-full flex items-center justify-center {tone[variant]}">
      <Icon name={variant === 'info' ? 'info' : 'warning'} />
    </div>
    <div class="min-w-0 flex-1 space-y-3">
      <p class="text-sm text-gray-600 dark:text-gray-300 whitespace-pre-line pt-2.5">{options.message}</p>
      {#if options.requireText}
        <div>
          <label for="confirm-typed" class="label">{m.confirm_type_to_continue({ text: options.requireText })}</label>
          <input id="confirm-typed" class="input font-mono" bind:value={typed} autocomplete="off" spellcheck="false" />
        </div>
      {/if}
    </div>
  </div>
  {#snippet footer()}
    <button type="button" onclick={() => closeDialog(false)} class="btn btn-secondary">
      {options.cancelText || m.common_cancel()}
    </button>
    <button type="button" onclick={() => closeDialog(true)} class={confirmClass[variant]} disabled={blocked}>
      {options.confirmText || m.common_confirm()}
    </button>
  {/snippet}
</Dialog>

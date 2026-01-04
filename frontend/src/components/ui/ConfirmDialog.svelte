<script lang="ts" module>
  import { writable } from 'svelte/store';

  export interface ConfirmOptions {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    variant?: 'danger' | 'warning' | 'info';
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
  let dialogRef = $state<HTMLDivElement | null>(null);
  let open = $state(false);
  let options = $state<ConfirmOptions>({ title: '', message: '' });

  dialogStore.subscribe((s) => {
    open = s.open;
    options = s.options;
  });

  function handleConfirm() {
    closeDialog(true);
  }

  function handleCancel() {
    closeDialog(false);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      handleCancel();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleCancel();
    }
  }

  $effect(() => {
    if (open && dialogRef) {
      dialogRef.focus();
    }
  });

  const iconColors = {
    danger: 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400',
    warning: 'bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400',
    info: 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400',
  };

  const confirmButtonStyles = {
    danger: 'btn btn-danger',
    warning: 'btn bg-amber-600 hover:bg-amber-700 text-white',
    info: 'btn btn-primary',
  };
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="confirm-title"
    aria-describedby="confirm-message"
    tabindex="-1"
    bind:this={dialogRef}
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-md w-full overflow-hidden">
      <div class="p-6">
        <div class="flex items-start gap-4">
          <!-- Icon -->
          <div class="flex-shrink-0 w-10 h-10 rounded-full {iconColors[options.variant || 'danger']} flex items-center justify-center">
            {#if options.variant === 'warning'}
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            {:else if options.variant === 'info'}
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            {:else}
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            {/if}
          </div>

          <!-- Content -->
          <div class="flex-1 min-w-0">
            <h3 id="confirm-title" class="text-lg font-semibold text-gray-900 dark:text-white">
              {options.title}
            </h3>
            <p id="confirm-message" class="mt-2 text-sm text-gray-600 dark:text-gray-400">
              {options.message}
            </p>
          </div>
        </div>

        <!-- Actions -->
        <div class="mt-6 flex gap-3 justify-end">
          <button type="button" onclick={handleCancel} class="btn btn-secondary">
            {options.cancelText || 'Cancel'}
          </button>
          <button type="button" onclick={handleConfirm} class={confirmButtonStyles[options.variant || 'danger']}>
            {options.confirmText || 'Confirm'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

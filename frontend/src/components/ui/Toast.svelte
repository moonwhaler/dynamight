<script lang="ts" module>
  import { writable } from 'svelte/store';

  export type ToastVariant = 'error' | 'success' | 'warning' | 'info';

  export interface ToastOptions {
    message: string;
    variant?: ToastVariant;
    duration?: number; // ms, 0 = no auto-dismiss
  }

  interface ToastState {
    visible: boolean;
    message: string;
    variant: ToastVariant;
    timeoutId: ReturnType<typeof setTimeout> | null;
  }

  const toastStore = writable<ToastState>({
    visible: false,
    message: '',
    variant: 'info',
    timeoutId: null,
  });

  export function showToast(options: ToastOptions): void {
    // Clear any existing timeout
    toastStore.update((s) => {
      if (s.timeoutId) {
        clearTimeout(s.timeoutId);
      }
      return s;
    });

    const duration = options.duration ?? 5000;

    const timeoutId =
      duration > 0
        ? setTimeout(() => {
            dismissToast();
          }, duration)
        : null;

    toastStore.set({
      visible: true,
      message: options.message,
      variant: options.variant || 'info',
      timeoutId,
    });
  }

  export function dismissToast(): void {
    toastStore.update((s) => {
      if (s.timeoutId) {
        clearTimeout(s.timeoutId);
      }
      return {
        visible: false,
        message: '',
        variant: 'info',
        timeoutId: null,
      };
    });
  }

  export { toastStore };
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let visible = $state(false);
  let message = $state('');
  let variant = $state<ToastVariant>('info');
  let el = $state<HTMLDivElement | null>(null);

  toastStore.subscribe((s) => {
    visible = s.visible;
    message = s.message;
    variant = s.variant;
  });

  // The toast lives in the browser's top layer (popover) so it shows above
  // open modal dialogs; showing it again moves it above a newer dialog.
  function raise() {
    if (!el?.showPopover) return;
    if (el.matches(':popover-open')) el.hidePopover();
    el.showPopover();
  }

  $effect(() => {
    if (!el?.showPopover) return;
    if (visible && message) raise();
    else if (el.matches(':popover-open')) el.hidePopover();
  });

  $effect(() => {
    const onDialog = () => {
      if (visible) raise();
    };
    window.addEventListener('dm:dialog-open', onDialog);
    return () => window.removeEventListener('dm:dialog-open', onDialog);
  });

  const styles: Record<ToastVariant, { icon: 'x-circle' | 'check-circle' | 'warning' | 'info'; tone: string }> = {
    error: { icon: 'x-circle', tone: 'text-red-600 dark:text-red-400' },
    success: { icon: 'check-circle', tone: 'text-green-600 dark:text-green-400' },
    warning: { icon: 'warning', tone: 'text-amber-600 dark:text-amber-400' },
    info: { icon: 'info', tone: 'text-primary-600 dark:text-primary-400' },
  };
</script>

<div
  bind:this={el}
  popover="manual"
  class="dm-toast"
  role={variant === 'error' ? 'alert' : 'status'}
  aria-live={variant === 'error' ? 'assertive' : 'polite'}
>
  {#if visible}
    <div
      class="flex items-start gap-3 pl-4 pr-2 py-3 rounded-xl shadow-xl border bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700"
    >
      <span class="shrink-0 mt-0.5 {styles[variant].tone}"><Icon name={styles[variant].icon} /></span>
      <p class="flex-1 text-sm text-gray-800 dark:text-gray-100 pt-0.5">{message}</p>
      <button type="button" onclick={dismissToast} class="btn btn-ghost btn-icon btn-sm shrink-0" aria-label={m.common_close()}>
        <Icon name="x" class="w-4 h-4" />
      </button>
    </div>
  {/if}
</div>

<style>
  .dm-toast {
    position: fixed;
    inset: auto 1rem calc(5rem + env(safe-area-inset-bottom)) 1rem;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    overflow: visible;
    max-width: 28rem;
    margin-inline: auto;
  }
  @media (min-width: 768px) {
    .dm-toast {
      inset: auto 1.5rem 1.5rem auto;
      width: 24rem;
    }
  }
  .dm-toast:popover-open {
    animation: toast-in 0.18s ease-out;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(0.5rem);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>

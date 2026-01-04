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
  let visible = $state(false);
  let message = $state('');
  let variant = $state<ToastVariant>('info');

  toastStore.subscribe((s) => {
    visible = s.visible;
    message = s.message;
    variant = s.variant;
  });

  const variantStyles: Record<ToastVariant, { bg: string; icon: string; border: string }> = {
    error: {
      bg: 'bg-red-50 dark:bg-red-900/30',
      icon: 'text-red-500 dark:text-red-400',
      border: 'border-red-200 dark:border-red-800',
    },
    success: {
      bg: 'bg-green-50 dark:bg-green-900/30',
      icon: 'text-green-500 dark:text-green-400',
      border: 'border-green-200 dark:border-green-800',
    },
    warning: {
      bg: 'bg-amber-50 dark:bg-amber-900/30',
      icon: 'text-amber-500 dark:text-amber-400',
      border: 'border-amber-200 dark:border-amber-800',
    },
    info: {
      bg: 'bg-blue-50 dark:bg-blue-900/30',
      icon: 'text-blue-500 dark:text-blue-400',
      border: 'border-blue-200 dark:border-blue-800',
    },
  };

  const textStyles: Record<ToastVariant, string> = {
    error: 'text-red-800 dark:text-red-200',
    success: 'text-green-800 dark:text-green-200',
    warning: 'text-amber-800 dark:text-amber-200',
    info: 'text-blue-800 dark:text-blue-200',
  };
</script>

{#if visible}
  <div
    class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 animate-toast-in max-w-md w-[calc(100%-2rem)]"
    role="alert"
    aria-live="assertive"
  >
    <div
      class="flex items-start gap-3 px-4 py-3 rounded-lg shadow-lg border {variantStyles[variant].bg} {variantStyles[variant].border}"
    >
      <!-- Icon -->
      <div class="flex-shrink-0 mt-0.5 {variantStyles[variant].icon}">
        {#if variant === 'error'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        {:else if variant === 'success'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        {:else if variant === 'warning'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        {:else}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        {/if}
      </div>

      <!-- Message -->
      <p class="flex-1 text-sm font-medium {textStyles[variant]}">
        {message}
      </p>

      <!-- Dismiss button -->
      <button
        onclick={dismissToast}
        class="flex-shrink-0 p-1 rounded-md hover:bg-black/10 dark:hover:bg-white/10 transition-colors {variantStyles[variant].icon}"
        aria-label="Dismiss"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  </div>
{/if}

<style>
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translate(-50%, 1rem);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }

  .animate-toast-in {
    animation: toast-in 0.2s ease-out;
  }
</style>

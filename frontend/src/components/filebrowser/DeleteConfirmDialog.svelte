<script lang="ts">
  import { authStore } from '$lib/stores/auth';
  import { fileBrowserStore } from '$lib/stores/fileBrowser';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    open: boolean;
    entryName: string;
    entryPath: string;
    isDirectory: boolean;
    onClose: () => void;
    onSuccess: () => void;
  }

  let { open, entryName, entryPath, isDirectory, onClose, onSuccess }: Props = $props();

  let password = $state('');
  let totpCode = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);

  let dialogRef = $state<HTMLDivElement | null>(null);

  // Check if user has 2FA enabled
  const needs2FA = $derived($authStore.user?.totp_enabled ?? false);

  function resetForm() {
    password = '';
    totpCode = '';
    error = null;
    loading = false;
  }

  function handleClose() {
    resetForm();
    onClose();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      handleClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleClose();
    }
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = null;

    if (!password) {
      error = String(m.error_field_required({ field: m.filebrowser_delete_password() }));
      return;
    }

    if (needs2FA && !totpCode) {
      error = String(m.error_field_required({ field: m.filebrowser_delete_totp() }));
      return;
    }

    loading = true;

    try {
      // Verify credentials
      const verified = await fileBrowserStore.verifyDeleteAccess(password, needs2FA ? totpCode : undefined);

      if (!verified) {
        error = String(m.error_delete_verification_failed());
        loading = false;
        return;
      }

      // Now perform the delete
      const result = await fileBrowserStore.deleteEntry(entryPath);

      if (result === 'success') {
        resetForm();
        onSuccess();
      } else if (result === 'verification_required') {
        error = String(m.error_delete_verification_required());
      } else {
        error = String(m.error_delete_failed());
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(m.error_delete_failed());
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open && dialogRef) {
      dialogRef.focus();
    }
  });

  $effect(() => {
    if (open) {
      resetForm();
    }
  });
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-[100] p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="delete-confirm-title"
    tabindex="-1"
    bind:this={dialogRef}
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl max-w-md w-full overflow-hidden">
      <form onsubmit={handleSubmit}>
        <div class="p-6">
          <div class="flex items-start gap-4">
            <!-- Warning Icon -->
            <div class="flex-shrink-0 w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 flex items-center justify-center">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </div>

            <!-- Content -->
            <div class="flex-1 min-w-0">
              <h3 id="delete-confirm-title" class="text-lg font-semibold text-gray-900 dark:text-white">
                {m.filebrowser_delete_confirm_title()}
              </h3>
              <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">
                {#if isDirectory}
                  {m.filebrowser_delete_confirm_message_folder({ name: entryName })}
                {:else}
                  {m.filebrowser_delete_confirm_message({ name: entryName })}
                {/if}
              </p>
            </div>
          </div>

          <!-- Form Fields -->
          <div class="mt-6 space-y-4">
            <!-- Password -->
            <div>
              <label for="delete-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {m.filebrowser_delete_password()}
              </label>
              <input
                type="password"
                id="delete-password"
                bind:value={password}
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                autocomplete="current-password"
                disabled={loading}
              />
            </div>

            <!-- TOTP Code (if 2FA enabled) -->
            {#if needs2FA}
              <div>
                <label for="delete-totp" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {m.filebrowser_delete_totp()}
                </label>
                <input
                  type="text"
                  id="delete-totp"
                  bind:value={totpCode}
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-transparent font-mono tracking-widest text-center"
                  maxlength="6"
                  inputmode="numeric"
                  pattern="[0-9]*"
                  autocomplete="one-time-code"
                  disabled={loading}
                />
              </div>
            {/if}

            <!-- Error Message -->
            {#if error}
              <div class="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                <p class="text-sm text-red-600 dark:text-red-400">{error}</p>
              </div>
            {/if}
          </div>

          <!-- Actions -->
          <div class="mt-6 flex gap-3 justify-end">
            <button
              type="button"
              onclick={handleClose}
              disabled={loading}
              class="btn btn-secondary"
            >
              {m.common_cancel()}
            </button>
            <button
              type="submit"
              disabled={loading || !password || (needs2FA && !totpCode)}
              class="btn btn-danger"
            >
              {#if loading}
                <svg class="w-4 h-4 animate-spin mr-2" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                {m.filebrowser_deleting()}
              {:else}
                {m.filebrowser_delete()}
              {/if}
            </button>
          </div>
        </div>
      </form>
    </div>
  </div>
{/if}

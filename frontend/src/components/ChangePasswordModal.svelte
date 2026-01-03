<script lang="ts">
  import { api } from '../lib/api';
  import PasswordStrength from './PasswordStrength.svelte';

  let { open = $bindable(false) } = $props();

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let loading = $state(false);
  let error = $state('');
  let success = $state(false);

  function reset() {
    currentPassword = '';
    newPassword = '';
    confirmPassword = '';
    error = '';
    success = false;
  }

  function close() {
    open = false;
    reset();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      close();
    }
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = '';

    if (newPassword.length < 8) {
      error = 'New password must be at least 8 characters';
      return;
    }

    if (newPassword !== confirmPassword) {
      error = 'Passwords do not match';
      return;
    }

    if (currentPassword === newPassword) {
      error = 'New password must be different from current password';
      return;
    }

    loading = true;
    try {
      await api.auth.changePassword(currentPassword, newPassword);
      success = true;
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to change password';
    } finally {
      loading = false;
    }
  }
</script>

{#if open}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="modal-title"
    tabindex="-1"
  >
    <div class="bg-white rounded-xl shadow-xl max-w-md w-full overflow-hidden">
      <div class="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
        <h2 id="modal-title" class="text-lg font-semibold text-gray-900">Change Password</h2>
        <button
          onclick={close}
          class="text-gray-400 hover:text-gray-600 transition-colors"
          aria-label="Close"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div class="p-6">
        {#if success}
          <div class="text-center py-4">
            <div class="w-12 h-12 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg class="w-6 h-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <h3 class="text-lg font-medium text-gray-900 mb-2">Password Changed</h3>
            <p class="text-sm text-gray-600 mb-6">Your password has been updated successfully.</p>
            <button onclick={close} class="btn btn-primary">Done</button>
          </div>
        {:else}
          <form onsubmit={handleSubmit} class="space-y-4">
            {#if error}
              <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg text-sm">
                {error}
              </div>
            {/if}

            <div>
              <label for="current-password" class="label">Current Password</label>
              <input
                id="current-password"
                type="password"
                required
                bind:value={currentPassword}
                class="input"
                placeholder="Enter current password"
                autocomplete="current-password"
              />
            </div>

            <div>
              <label for="new-password" class="label">New Password</label>
              <input
                id="new-password"
                type="password"
                required
                bind:value={newPassword}
                class="input"
                placeholder="Enter new password"
                autocomplete="new-password"
              />
              <PasswordStrength password={newPassword} />
              {#if !newPassword}
                <p class="text-xs text-gray-500 mt-1">Must be at least 8 characters</p>
              {/if}
            </div>

            <div>
              <label for="confirm-password" class="label">Confirm New Password</label>
              <input
                id="confirm-password"
                type="password"
                required
                bind:value={confirmPassword}
                class="input"
                placeholder="Confirm new password"
                autocomplete="new-password"
              />
            </div>

            <div class="flex gap-3 pt-2">
              <button type="button" onclick={close} class="btn btn-secondary flex-1">
                Cancel
              </button>
              <button type="submit" disabled={loading} class="btn btn-primary flex-1">
                {#if loading}
                  <span class="flex items-center justify-center gap-2">
                    <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                      <circle
                        class="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        stroke-width="4"
                        fill="none"
                      />
                      <path
                        class="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                      />
                    </svg>
                    Changing...
                  </span>
                {:else}
                  Change Password
                {/if}
              </button>
            </div>
          </form>
        {/if}
      </div>
    </div>
  </div>
{/if}

<script lang="ts">
  import { authStore } from '$lib/stores/auth';
  import { fileBrowserStore } from '$lib/stores/fileBrowser';
  import * as m from '$lib/paraglide/messages.js';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from '../ui/Icon.svelte';
  import Spinner from '../ui/Spinner.svelte';

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
    if (open) {
      resetForm();
    }
  });
</script>

<Dialog
  bind:open={() => open, (v) => { if (!v) handleClose(); }}
  title={m.filebrowser_delete_confirm_title()}
  size="sm"
  role="alertdialog"
  dismissible={!loading}
>
  <form id="delete-entry-form" onsubmit={handleSubmit} class="space-y-4">
    <div class="flex items-start gap-3">
      <div class="shrink-0 w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 flex items-center justify-center">
        <Icon name="trash" />
      </div>
      <p class="text-sm text-gray-600 dark:text-gray-300 pt-1">
        {#if isDirectory}
          {m.filebrowser_delete_confirm_message_folder({ name: entryName })}
        {:else}
          {m.filebrowser_delete_confirm_message({ name: entryName })}
        {/if}
      </p>
    </div>

    <div>
      <label for="delete-password" class="label">{m.filebrowser_delete_password()}</label>
      <input type="password" id="delete-password" bind:value={password} class="input" autocomplete="current-password" disabled={loading} />
    </div>

    {#if needs2FA}
      <div>
        <label for="delete-totp" class="label">{m.filebrowser_delete_totp()}</label>
        <input
          type="text"
          id="delete-totp"
          bind:value={totpCode}
          class="input font-mono tracking-widest text-center"
          maxlength="6"
          inputmode="numeric"
          pattern="[0-9]*"
          autocomplete="one-time-code"
          disabled={loading}
        />
      </div>
    {/if}

    {#if error}
      <p class="p-3 bg-red-50 dark:bg-red-900/20 rounded-lg text-sm text-red-700 dark:text-red-400" role="alert">{error}</p>
    {/if}
  </form>

  {#snippet footer()}
    <button type="button" onclick={handleClose} disabled={loading} class="btn btn-secondary">{m.common_cancel()}</button>
    <button type="submit" form="delete-entry-form" disabled={loading || !password || (needs2FA && !totpCode)} class="btn btn-danger">
      {#if loading}<Spinner />{m.filebrowser_deleting()}{:else}{m.filebrowser_delete()}{/if}
    </button>
  {/snippet}
</Dialog>

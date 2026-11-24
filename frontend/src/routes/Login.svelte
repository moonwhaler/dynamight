<script lang="ts">
  import { authStore } from '../lib/stores/auth';
  import TotpVerification from '../components/TotpVerification.svelte';
  import Logo from '../components/shell/Logo.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let username = $state('');
  let password = $state('');
  let loading = $state(false);

  async function handleSubmit(e: Event) {
    e.preventDefault();
    loading = true;
    await authStore.login(username, password);
    loading = false;
  }
</script>

{#if $authStore.pendingTotpSession}
  <TotpVerification />
{:else}
  <div class="min-h-[100dvh] flex flex-col items-center justify-center bg-gray-50 dark:bg-gray-900 px-4 py-12 pt-safe pb-safe">
    <div class="w-full max-w-sm">
      <div class="flex justify-center mb-10"><Logo large /></div>
      <h1 class="font-display text-[1.9rem] leading-tight text-gray-900 dark:text-white">{m.auth_sign_in()}</h1>
      <p class="mt-1.5 text-sm text-gray-500 dark:text-gray-400">{m.auth_sign_in_description()}</p>

      <form class="mt-8 space-y-5" onsubmit={handleSubmit}>
        {#if $authStore.error}
          <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
            {$authStore.error}
          </div>
        {/if}

        <div class="space-y-4">
          <div>
            <label for="username" class="label">{m.auth_username()}</label>
            <input
              id="username"
              name="username"
              type="text"
              autocomplete="username"
              autocapitalize="none"
              required
              bind:value={username}
              class="input"
            />
          </div>

          <div>
            <label for="password" class="label">{m.auth_password()}</label>
            <input
              id="password"
              name="password"
              type="password"
              autocomplete="current-password"
              required
              bind:value={password}
              class="input"
            />
          </div>
        </div>

        <button type="submit" disabled={loading} class="w-full btn btn-lg btn-primary">
          {#if loading}
            <Spinner size="w-5 h-5" />{m.auth_signing_in()}
          {:else}
            {m.auth_sign_in()}
          {/if}
        </button>
      </form>
    </div>
  </div>
{/if}

<script lang="ts">
  import { authStore } from '../lib/stores/auth';

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

<div class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
  <div class="max-w-md w-full space-y-8">
    <div class="text-center">
      <div
        class="mx-auto w-16 h-16 bg-primary-600 rounded-2xl flex items-center justify-center mb-4"
      >
        <svg class="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
          />
        </svg>
      </div>
      <h2 class="text-3xl font-bold text-gray-900 dark:text-white">Dynamight</h2>
      <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">Sign in to manage your backups</p>
    </div>

    <form class="mt-8 space-y-6" onsubmit={handleSubmit}>
      {#if $authStore.error}
        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg text-sm">
          {$authStore.error}
        </div>
      {/if}

      <div class="space-y-4">
        <div>
          <label for="username" class="label">Username</label>
          <input
            id="username"
            name="username"
            type="text"
            required
            bind:value={username}
            class="input"
            placeholder="admin"
          />
        </div>

        <div>
          <label for="password" class="label">Password</label>
          <input
            id="password"
            name="password"
            type="password"
            required
            bind:value={password}
            class="input"
            placeholder="Enter your password"
          />
        </div>
      </div>

      <button type="submit" disabled={loading} class="w-full btn btn-primary py-3">
        {#if loading}
          <span class="flex items-center justify-center gap-2">
            <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24">
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
            Signing in...
          </span>
        {:else}
          Sign in
        {/if}
      </button>
    </form>
  </div>
</div>

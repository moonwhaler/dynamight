<script lang="ts">
  import { authStore } from '../lib/stores/auth';
  import PasswordStrength from '../components/PasswordStrength.svelte';

  let username = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let loading = $state(false);
  let validationError = $state('');

  function validate(): boolean {
    validationError = '';

    if (username.trim().length < 3) {
      validationError = 'Username must be at least 3 characters';
      return false;
    }

    if (password.length < 8) {
      validationError = 'Password must be at least 8 characters';
      return false;
    }

    if (password !== confirmPassword) {
      validationError = 'Passwords do not match';
      return false;
    }

    return true;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!validate()) return;

    loading = true;
    const success = await authStore.setup(username.trim(), password);
    loading = false;

    if (success) {
      // Navigate to dashboard after successful setup
      window.location.hash = '#/';
    }
  }
</script>

<div class="min-h-screen flex items-center justify-center bg-gray-50 py-12 px-4 sm:px-6 lg:px-8">
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
      <h2 class="text-3xl font-bold text-gray-900">Welcome to Dynamight</h2>
      <p class="mt-2 text-sm text-gray-600">Create your administrator account to get started</p>
    </div>

    <form class="mt-8 space-y-6" onsubmit={handleSubmit}>
      {#if validationError || $authStore.error}
        <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg text-sm">
          {validationError || $authStore.error}
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
            placeholder="Choose a username"
            autocomplete="username"
          />
          <p class="mt-1 text-xs text-gray-500">At least 3 characters</p>
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
            placeholder="Choose a password"
            autocomplete="new-password"
          />
          <PasswordStrength {password} />
          {#if !password}
            <p class="mt-1 text-xs text-gray-500">At least 8 characters</p>
          {/if}
        </div>

        <div>
          <label for="confirmPassword" class="label">Confirm Password</label>
          <input
            id="confirmPassword"
            name="confirmPassword"
            type="password"
            required
            bind:value={confirmPassword}
            class="input"
            placeholder="Confirm your password"
            autocomplete="new-password"
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
            Creating account...
          </span>
        {:else}
          Create Account
        {/if}
      </button>
    </form>

    <div class="text-center">
      <p class="text-xs text-gray-500">
        This will be the only administrator account. You can change the password later in settings.
      </p>
    </div>
  </div>
</div>

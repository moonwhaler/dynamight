<script lang="ts">
  import { authStore } from '../lib/stores/auth';

  let code = $state('');
  let recoveryCode = $state('');
  let useRecoveryCode = $state(false);
  let loading = $state(false);
  let codesWarning = $state<number | null>(null);

  async function handleSubmit(e: Event) {
    e.preventDefault();
    loading = true;

    if (useRecoveryCode) {
      const result = await authStore.validateRecoveryCode(recoveryCode);
      if (result.success && result.codesRemaining !== undefined && result.codesRemaining < 3) {
        codesWarning = result.codesRemaining;
      }
    } else {
      await authStore.validateTotp(code);
    }

    loading = false;
  }

  function goBack() {
    authStore.clearPendingSession();
  }

  function handleCodeInput(e: Event) {
    const input = e.target as HTMLInputElement;
    input.value = input.value.replace(/\D/g, '').slice(0, 6);
    code = input.value;
  }

  function handleRecoveryInput(e: Event) {
    const input = e.target as HTMLInputElement;
    // Allow alphanumeric and dashes
    input.value = input.value.toUpperCase().replace(/[^A-Z0-9-]/g, '');
    recoveryCode = input.value;
  }

  function toggleMode() {
    useRecoveryCode = !useRecoveryCode;
    authStore.clearError();
    code = '';
    recoveryCode = '';
  }
</script>

<div class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 px-4">
  <div class="max-w-md w-full">
    <div class="bg-white dark:bg-gray-800 rounded-2xl shadow-xl p-8">
      <!-- Header -->
      <div class="text-center mb-8">
        <div class="w-16 h-16 bg-primary-100 dark:bg-primary-900/40 rounded-full flex items-center justify-center mx-auto mb-4">
          <svg class="w-8 h-8 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
          </svg>
        </div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">
          {useRecoveryCode ? 'Recovery Code' : 'Two-Factor Authentication'}
        </h1>
        <p class="text-gray-600 dark:text-gray-400">
          {useRecoveryCode
            ? 'Enter one of your recovery codes'
            : 'Enter the 6-digit code from your authenticator app'}
        </p>
      </div>

      <!-- Error message -->
      {#if $authStore.error}
        <div class="mb-6 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-xl text-sm">
          {$authStore.error}
        </div>
      {/if}

      <!-- Codes warning -->
      {#if codesWarning !== null}
        <div class="mb-6 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-400 px-4 py-3 rounded-xl text-sm">
          <strong>Warning:</strong> You only have {codesWarning} recovery code{codesWarning !== 1 ? 's' : ''} remaining.
        </div>
      {/if}

      <form onsubmit={handleSubmit}>
        {#if useRecoveryCode}
          <div class="mb-6">
            <label for="recovery-code" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Recovery Code
            </label>
            <input
              id="recovery-code"
              type="text"
              value={recoveryCode}
              oninput={handleRecoveryInput}
              placeholder="XXXX-XXXX-XXXX"
              class="input text-center text-lg tracking-wider font-mono"
              autocomplete="off"
            />
          </div>
        {:else}
          <div class="mb-6">
            <label for="totp-code" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Authentication Code
            </label>
            <input
              id="totp-code"
              type="text"
              inputmode="numeric"
              maxlength="6"
              value={code}
              oninput={handleCodeInput}
              placeholder="000000"
              class="input text-center text-2xl tracking-widest font-mono"
              autocomplete="one-time-code"
            />
          </div>
        {/if}

        <button
          type="submit"
          disabled={loading || (useRecoveryCode ? recoveryCode.length < 10 : code.length !== 6)}
          class="btn btn-lg btn-primary w-full"
        >
          {#if loading}
            <span class="flex items-center justify-center gap-2">
              <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              Verifying...
            </span>
          {:else}
            Verify
          {/if}
        </button>
      </form>

      <div class="mt-6 flex flex-col items-center gap-3">
        <button
          onclick={toggleMode}
          class="text-sm text-primary-600 dark:text-primary-400 hover:underline"
        >
          {useRecoveryCode ? 'Use authenticator app instead' : 'Use a recovery code'}
        </button>

        <button
          onclick={goBack}
          class="text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
        >
          Back to login
        </button>
      </div>
    </div>
  </div>
</div>

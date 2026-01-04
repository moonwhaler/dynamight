<script lang="ts">
  import { api } from '../lib/api';

  type SetupStep = 'initial' | 'qr' | 'verify' | 'recovery' | 'complete';

  let { onEnabled = () => {} } = $props<{ onEnabled?: () => void }>();

  let step = $state<SetupStep>('initial');
  let secret = $state('');
  let qrCode = $state('');
  let otpauthUrl = $state('');
  let verificationCode = $state('');
  let recoveryCodes = $state<string[]>([]);
  let loading = $state(false);
  let error = $state('');
  let showManualEntry = $state(false);
  let copiedCodes = $state(false);
  let confirmedSaved = $state(false);

  async function startSetup() {
    loading = true;
    error = '';
    try {
      const response = await api.auth.totpSetup();
      secret = response.secret;
      qrCode = response.qr_code;
      otpauthUrl = response.otpauth_url;
      step = 'qr';
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to start setup';
    }
    loading = false;
  }

  async function verifyCode() {
    if (verificationCode.length !== 6) {
      error = 'Please enter a 6-digit code';
      return;
    }

    loading = true;
    error = '';
    try {
      const response = await api.auth.totpEnable(verificationCode, secret);
      recoveryCodes = response.recovery_codes;
      step = 'recovery';
    } catch (e) {
      error = e instanceof Error ? e.message : 'Verification failed';
    }
    loading = false;
  }

  function copyRecoveryCodes() {
    const codesText = recoveryCodes.join('\n');
    navigator.clipboard.writeText(codesText);
    copiedCodes = true;
    setTimeout(() => copiedCodes = false, 2000);
  }

  function finishSetup() {
    step = 'complete';
    onEnabled();
  }

  function handleCodeInput(e: Event) {
    const input = e.target as HTMLInputElement;
    // Only allow digits
    input.value = input.value.replace(/\D/g, '').slice(0, 6);
    verificationCode = input.value;
  }
</script>

<div class="space-y-6">
  {#if step === 'initial'}
    <!-- Initial state - explain 2FA -->
    <div class="text-center py-8">
      <div class="w-20 h-20 bg-primary-100 dark:bg-primary-900/40 rounded-full flex items-center justify-center mx-auto mb-6">
        <svg class="w-10 h-10 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
        </svg>
      </div>
      <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3">Two-Factor Authentication</h3>
      <p class="text-gray-600 dark:text-gray-400 max-w-md mx-auto mb-8">
        Add an extra layer of security to your account. You'll need an authenticator app like Google Authenticator or Aegis.
      </p>
      <button onclick={startSetup} disabled={loading} class="btn btn-primary px-8 py-2.5">
        {#if loading}
          <span class="flex items-center gap-2">
            <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            Setting up...
          </span>
        {:else}
          Enable Two-Factor Authentication
        {/if}
      </button>
    </div>

  {:else if step === 'qr'}
    <!-- QR Code step -->
    <div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Scan QR Code</h3>
      <p class="text-gray-600 dark:text-gray-400 mb-6">
        Open your authenticator app and scan this QR code to add your account.
      </p>

      {#if error}
        <div class="mb-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400 px-4 py-3 rounded-xl text-sm">
          {error}
        </div>
      {/if}

      <div class="flex flex-col items-center">
        <div class="bg-white p-4 rounded-xl shadow-sm border border-gray-200 mb-6">
          <img src={qrCode} alt="2FA QR Code" class="w-48 h-48" />
        </div>

        <button
          onclick={() => showManualEntry = !showManualEntry}
          class="text-sm text-primary-600 dark:text-primary-400 hover:underline mb-6"
        >
          {showManualEntry ? 'Hide manual entry' : "Can't scan? Enter code manually"}
        </button>

        {#if showManualEntry}
          <div class="w-full bg-gray-50 dark:bg-gray-900/50 rounded-xl p-4 mb-6">
            <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">Manual entry key:</p>
            <code class="block bg-white dark:bg-gray-800 px-3 py-2 rounded-lg text-sm font-mono break-all border border-gray-200 dark:border-gray-700">
              {secret}
            </code>
          </div>
        {/if}

        <div class="w-full max-w-xs">
          <label for="verification-code" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Enter the 6-digit code from your app
          </label>
          <input
            id="verification-code"
            type="text"
            inputmode="numeric"
            maxlength="6"
            placeholder="000000"
            value={verificationCode}
            oninput={handleCodeInput}
            class="input text-center text-2xl tracking-widest font-mono"
            autocomplete="one-time-code"
          />
        </div>

        <div class="flex gap-3 mt-6">
          <button onclick={() => step = 'initial'} class="btn btn-secondary px-6">
            Back
          </button>
          <button onclick={verifyCode} disabled={loading || verificationCode.length !== 6} class="btn btn-primary px-6">
            {#if loading}
              <span class="flex items-center gap-2">
                <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                </svg>
                Verifying...
              </span>
            {:else}
              Verify & Continue
            {/if}
          </button>
        </div>
      </div>
    </div>

  {:else if step === 'recovery'}
    <!-- Recovery codes step -->
    <div>
      <div class="flex items-center gap-3 mb-4">
        <div class="w-10 h-10 bg-amber-100 dark:bg-amber-900/40 rounded-xl flex items-center justify-center">
          <svg class="w-5 h-5 text-amber-600 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Save Your Recovery Codes</h3>
          <p class="text-sm text-gray-600 dark:text-gray-400">Store these codes in a safe place</p>
        </div>
      </div>

      <div class="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-xl p-4 mb-6">
        <p class="text-sm text-amber-800 dark:text-amber-300">
          <strong>Important:</strong> If you lose access to your authenticator app, you can use these recovery codes to sign in. Each code can only be used once.
        </p>
      </div>

      <div class="bg-gray-50 dark:bg-gray-900/50 rounded-xl p-4 mb-4">
        <div class="grid grid-cols-2 gap-2">
          {#each recoveryCodes as code}
            <code class="bg-white dark:bg-gray-800 px-3 py-2 rounded-lg text-sm font-mono text-center border border-gray-200 dark:border-gray-700">
              {code}
            </code>
          {/each}
        </div>
      </div>

      <button onclick={copyRecoveryCodes} class="w-full btn btn-secondary mb-6">
        {#if copiedCodes}
          <span class="flex items-center justify-center gap-2">
            <svg class="w-4 h-4 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            Copied!
          </span>
        {:else}
          <span class="flex items-center justify-center gap-2">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            Copy All Codes
          </span>
        {/if}
      </button>

      <label class="flex items-start gap-3 p-4 bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmedSaved}
          class="mt-0.5 rounded text-primary-600"
        />
        <span class="text-sm text-gray-700 dark:text-gray-300">
          I have saved these recovery codes in a safe place
        </span>
      </label>

      <div class="mt-6">
        <button onclick={finishSetup} disabled={!confirmedSaved} class="btn btn-primary w-full py-2.5 disabled:opacity-50 disabled:cursor-not-allowed">
          Complete Setup
        </button>
      </div>
    </div>

  {:else if step === 'complete'}
    <!-- Success state -->
    <div class="text-center py-8">
      <div class="w-20 h-20 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center mx-auto mb-6">
        <svg class="w-10 h-10 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
      </div>
      <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-3">Two-Factor Authentication Enabled</h3>
      <p class="text-gray-600 dark:text-gray-400 max-w-md mx-auto">
        Your account is now protected with two-factor authentication. You'll need to enter a code from your authenticator app when you sign in.
      </p>
    </div>
  {/if}
</div>

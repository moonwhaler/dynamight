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
  let showEnlargedQr = $state(false);

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

<div class="space-y-4">
  {#if step === 'initial'}
    <!-- Initial state - explain 2FA -->
    <div class="text-center py-4">
      <div class="w-12 h-12 bg-primary-100 dark:bg-primary-900/40 rounded-full flex items-center justify-center mx-auto mb-3">
        <svg class="w-6 h-6 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
        </svg>
      </div>
      <h3 class="text-base font-semibold text-gray-900 dark:text-white mb-1">Enable Two-Factor Authentication</h3>
      <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
        Use an authenticator app like Google Authenticator or Aegis.
      </p>
      <button onclick={startSetup} disabled={loading} class="btn btn-primary px-6 py-2">
        {#if loading}
          <span class="flex items-center gap-2">
            <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            Setting up...
          </span>
        {:else}
          Get Started
        {/if}
      </button>
    </div>

  {:else if step === 'qr'}
    <!-- QR Code step -->
    <div>
      {#if error}
        <div class="mb-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-3 py-2 rounded-lg text-sm">
          {error}
        </div>
      {/if}

      <div class="flex flex-col sm:flex-row gap-4 items-center sm:items-start">
        <!-- QR Code -->
        <div class="flex-shrink-0">
          <button
            type="button"
            onclick={() => showEnlargedQr = true}
            class="bg-white p-2 rounded-lg shadow-sm border border-gray-200 cursor-pointer transition-all duration-200 hover:shadow-md hover:scale-[1.02] active:scale-[0.98] focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2"
            title="Click to enlarge"
          >
            <img src={qrCode} alt="2FA QR Code" class="w-32 h-32 sm:w-36 sm:h-36" />
          </button>
          <p class="mt-1.5 text-[10px] text-gray-400 dark:text-gray-500 text-center">Tap to enlarge</p>
          <button
            onclick={() => showManualEntry = !showManualEntry}
            class="mt-1 text-xs text-primary-600 dark:text-primary-400 hover:underline w-full text-center"
          >
            {showManualEntry ? 'Hide key' : 'Manual entry'}
          </button>
        </div>

        <!-- Verification form -->
        <div class="flex-1 w-full">
          <p class="text-sm text-gray-600 dark:text-gray-400 mb-3">
            Scan with your authenticator app, then enter the code below.
          </p>

          {#if showManualEntry}
            <div class="bg-gray-50 dark:bg-gray-900/50 rounded-lg p-2 mb-3">
              <p class="text-xs text-gray-500 dark:text-gray-400 mb-1">Secret key:</p>
              <code class="block text-xs font-mono break-all text-gray-700 dark:text-gray-300">
                {secret}
              </code>
            </div>
          {/if}

          <div>
            <label for="verification-code" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
              6-digit code
            </label>
            <input
              id="verification-code"
              type="text"
              inputmode="numeric"
              maxlength="6"
              placeholder="000000"
              value={verificationCode}
              oninput={handleCodeInput}
              class="input text-center text-xl tracking-widest font-mono"
              autocomplete="one-time-code"
            />
          </div>

          <div class="flex gap-2 mt-4">
            <button onclick={() => step = 'initial'} class="btn btn-secondary px-4 py-2">
              Back
            </button>
            <button onclick={verifyCode} disabled={loading || verificationCode.length !== 6} class="btn btn-primary flex-1 py-2">
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
          </div>
        </div>
      </div>
    </div>

  {:else if step === 'recovery'}
    <!-- Recovery codes step -->
    <div>
      <div class="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg p-3 mb-4">
        <p class="text-sm text-amber-800 dark:text-amber-300">
          <strong>Save these recovery codes!</strong> Use them to sign in if you lose access to your authenticator.
        </p>
      </div>

      <div class="bg-gray-50 dark:bg-gray-900/50 rounded-lg p-3 mb-3">
        <div class="grid grid-cols-2 gap-1.5">
          {#each recoveryCodes as code}
            <code class="bg-white dark:bg-gray-800 px-2 py-1.5 rounded text-xs font-mono text-center border border-gray-200 dark:border-gray-700">
              {code}
            </code>
          {/each}
        </div>
      </div>

      <button onclick={copyRecoveryCodes} class="w-full btn btn-secondary py-2 mb-4 text-sm">
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
            Copy Codes
          </span>
        {/if}
      </button>

      <label class="flex items-center gap-2.5 p-3 bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmedSaved}
          class="rounded text-primary-600"
        />
        <span class="text-sm text-gray-700 dark:text-gray-300">
          I've saved these codes safely
        </span>
      </label>

      <button onclick={finishSetup} disabled={!confirmedSaved} class="btn btn-primary w-full py-2 mt-4 disabled:opacity-50">
        Complete Setup
      </button>
    </div>

  {:else if step === 'complete'}
    <!-- Success state -->
    <div class="text-center py-4">
      <div class="w-12 h-12 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center mx-auto mb-3">
        <svg class="w-6 h-6 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
      </div>
      <h3 class="text-base font-semibold text-gray-900 dark:text-white mb-1">2FA Enabled</h3>
      <p class="text-sm text-gray-500 dark:text-gray-400">
        Your account is now protected with two-factor authentication.
      </p>
    </div>
  {/if}
</div>

<!-- Enlarged QR Code Modal -->
{#if showEnlargedQr}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4"
    role="dialog"
    aria-modal="true"
    aria-label="Enlarged QR Code"
  >
    <!-- Backdrop -->
    <button
      type="button"
      class="absolute inset-0 bg-black/60 backdrop-blur-sm animate-fade-in"
      onclick={() => showEnlargedQr = false}
      aria-label="Close enlarged QR code"
    ></button>

    <!-- QR Code Container -->
    <div class="relative animate-scale-in">
      <button
        type="button"
        onclick={() => showEnlargedQr = false}
        class="bg-white p-4 sm:p-6 rounded-2xl shadow-2xl cursor-pointer transition-transform duration-200 hover:scale-[1.02] active:scale-[0.98] focus:outline-none focus:ring-4 focus:ring-primary-500/50"
      >
        <img src={qrCode} alt="2FA QR Code" class="w-64 h-64 sm:w-80 sm:h-80" />
      </button>
      <p class="text-center text-white/80 text-sm mt-3">Tap anywhere to close</p>
    </div>
  </div>
{/if}

<style>
  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scale-in {
    from {
      opacity: 0;
      transform: scale(0.9);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .animate-fade-in {
    animation: fade-in 0.2s ease-out forwards;
  }

  .animate-scale-in {
    animation: scale-in 0.25s ease-out forwards;
  }
</style>

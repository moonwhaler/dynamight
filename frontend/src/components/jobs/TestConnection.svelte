<script lang="ts">
  import { api } from '../../lib/api';
  import type { DestinationConfig } from '../../lib/types';
  import * as m from '$lib/paraglide/messages.js';

  type TestState = 'idle' | 'testing' | 'success' | 'error';

  let {
    destination,
    credentialId = null,
    disabled = false,
  }: {
    destination: DestinationConfig;
    credentialId: number | null;
    disabled?: boolean;
  } = $props();

  let testState: TestState = $state('idle');
  let resultMessage = $state('');
  let resultDetails: string | null = $state(null);

  async function testConnection() {
    testState = 'testing';
    resultMessage = '';
    resultDetails = null;

    try {
      const result = await api.providers.testConnection(destination, credentialId);
      if (result.success) {
        testState = 'success';
        resultMessage = result.message;
        resultDetails = result.details || null;
      } else {
        testState = 'error';
        resultMessage = result.message;
        resultDetails = result.details || null;
      }
    } catch (e) {
      testState = 'error';
      resultMessage = e instanceof Error ? e.message : m.test_connection_error_generic();
    }

    // Auto-reset success state after 5 seconds
    if (testState === 'success') {
      setTimeout(() => {
        if (testState === 'success') {
          testState = 'idle';
          resultMessage = '';
          resultDetails = null;
        }
      }, 5000);
    }
  }

  // Reset state when destination or credential changes
  $effect(() => {
    // Access the reactive values
    const _dest = destination;
    const _cred = credentialId;
    // Reset to idle
    testState = 'idle';
    resultMessage = '';
    resultDetails = null;
  });
</script>

<div class="space-y-2">
  <button
    type="button"
    onclick={testConnection}
    disabled={disabled || testState === 'testing'}
    class="btn btn-secondary inline-flex items-center gap-2 text-sm"
  >
    {#if testState === 'testing'}
      <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
        <path
          class="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
        />
      </svg>
      {m.test_connection_testing()}
    {:else if testState === 'success'}
      <svg class="w-4 h-4 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
      </svg>
      {m.test_connection_success()}
    {:else if testState === 'error'}
      <svg class="w-4 h-4 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
      {m.test_connection_retry()}
    {:else}
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M13 10V3L4 14h7v7l9-11h-7z"
        />
      </svg>
      {m.test_connection_button()}
    {/if}
  </button>

  {#if resultMessage}
    <div
      class="flex items-start gap-2 p-3 rounded-lg text-sm transition-all {testState === 'success'
        ? 'bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-300'
        : 'bg-red-50 dark:bg-red-900/20 text-red-800 dark:text-red-300'}"
    >
      {#if testState === 'success'}
        <svg class="w-5 h-5 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      {:else}
        <svg class="w-5 h-5 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      {/if}
      <div class="flex-1 min-w-0">
        <p class="font-medium">{resultMessage}</p>
        {#if resultDetails}
          <p class="mt-1 text-xs opacity-80">{resultDetails}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

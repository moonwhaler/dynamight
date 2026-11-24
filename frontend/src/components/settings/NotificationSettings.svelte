<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../../lib/api';
  import type {
    NotificationEvents,
    NotificationSettings,
    NotificationSettingsUpdate,
    NotificationTestResult,
  } from '../../lib/types';
  import { showToast } from '../ui/Toast.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let testing = $state(false);
  let testResults = $state<NotificationTestResult[] | null>(null);

  let ntfyEnabled = $state(false);
  let ntfyServer = $state('https://ntfy.sh');
  let ntfyTopic = $state('');
  // Secrets are write-only: the input holds a new value, `*Set` says whether
  // one is stored and `*Remove` clears it on save
  let ntfyToken = $state('');
  let ntfyTokenSet = $state(false);
  let ntfyTokenRemove = $state(false);

  let webhookEnabled = $state(false);
  let webhookUrl = $state('');
  let webhookAuth = $state('');
  let webhookAuthSet = $state(false);
  let webhookAuthRemove = $state(false);

  let events = $state<NotificationEvents>({ failed: true, overdue: true, recovered: true, success: false });
  let baseUrl = $state('');

  const eventOptions: { key: keyof NotificationEvents; label: () => string; desc: () => string }[] = [
    { key: 'failed', label: () => m.notify_event_failed(), desc: () => m.notify_event_failed_desc() },
    { key: 'overdue', label: () => m.notify_event_overdue(), desc: () => m.notify_event_overdue_desc() },
    { key: 'recovered', label: () => m.notify_event_recovered(), desc: () => m.notify_event_recovered_desc() },
    { key: 'success', label: () => m.notify_event_success(), desc: () => m.notify_event_success_desc() },
  ];

  function apply(s: NotificationSettings) {
    ntfyEnabled = s.ntfy.enabled;
    ntfyServer = s.ntfy.server || 'https://ntfy.sh';
    ntfyTopic = s.ntfy.topic;
    ntfyToken = '';
    ntfyTokenSet = s.ntfy.token_set;
    ntfyTokenRemove = false;
    webhookEnabled = s.webhook.enabled;
    webhookUrl = s.webhook.url;
    webhookAuth = '';
    webhookAuthSet = s.webhook.auth_header_set;
    webhookAuthRemove = false;
    events = { ...s.events };
    baseUrl = s.base_url;
  }

  function secret(value: string, remove: boolean): string | null {
    if (remove) return '';
    return value.trim() ? value.trim() : null;
  }

  function request(): NotificationSettingsUpdate {
    return {
      ntfy: { enabled: ntfyEnabled, server: ntfyServer, topic: ntfyTopic, token: secret(ntfyToken, ntfyTokenRemove) },
      webhook: { enabled: webhookEnabled, url: webhookUrl, auth_header: secret(webhookAuth, webhookAuthRemove) },
      events: { ...events },
      base_url: baseUrl,
    };
  }

  async function load() {
    loading = true;
    loadError = null;
    try {
      apply(await api.notifications.get());
    } catch (e) {
      loadError = e instanceof Error ? e.message : m.error_failed_load();
    } finally {
      loading = false;
    }
  }

  async function save() {
    saving = true;
    try {
      apply(await api.notifications.update(request()));
      showToast({ message: m.notify_saved(), variant: 'success' });
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      saving = false;
    }
  }

  async function sendTest() {
    testing = true;
    testResults = null;
    try {
      testResults = (await api.notifications.test(request())).results;
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      testing = false;
    }
  }

  onMount(load);
</script>

<div class="mb-6">
  <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{m.notify_title()}</h3>
  <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{m.notify_description()}</p>
</div>

{#if loading}
  <div class="flex justify-center py-8"><Spinner size="w-6 h-6" /></div>
{:else if loadError}
  <div class="p-4 rounded-xl bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-400">
    {loadError}
    <button type="button" class="ml-2 underline" onclick={load}>{m.common_retry()}</button>
  </div>
{:else}
  <form
    class="space-y-6"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <!-- Events -->
    <div class="space-y-3">
      <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">{m.notify_events()}</h4>
      {#each eventOptions as option (option.key)}
        <label class="flex items-start gap-3 p-3 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer">
          <input type="checkbox" bind:checked={events[option.key]} class="mt-0.5 rounded text-primary-600" />
          <div class="min-w-0">
            <div class="text-sm font-medium text-gray-900 dark:text-white">{option.label()}</div>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{option.desc()}</p>
          </div>
        </label>
      {/each}
    </div>

    <!-- ntfy -->
    <div class="space-y-3 pt-4 border-t border-gray-200 dark:border-gray-700">
      <label class="flex items-start gap-4 cursor-pointer">
        <div class="relative flex items-center">
          <input type="checkbox" bind:checked={ntfyEnabled} class="peer sr-only" />
          <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
          <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
        </div>
        <div class="flex-1 min-w-0">
          <div class="font-medium text-gray-900 dark:text-white text-sm">{m.notify_ntfy()}</div>
          <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.notify_ntfy_desc()}</p>
        </div>
      </label>

      {#if ntfyEnabled}
        <div class="grid gap-3 sm:grid-cols-2">
          <div>
            <label for="ntfy-server" class="label">{m.notify_ntfy_server()}</label>
            <input id="ntfy-server" type="url" class="input" bind:value={ntfyServer} placeholder="https://ntfy.sh" />
          </div>
          <div>
            <label for="ntfy-topic" class="label">{m.notify_ntfy_topic()}</label>
            <input id="ntfy-topic" type="text" class="input font-mono" bind:value={ntfyTopic} autocomplete="off" />
          </div>
          <p class="sm:col-span-2 text-xs text-gray-500 dark:text-gray-400 -mt-1">{m.notify_ntfy_topic_help()}</p>
          <div class="sm:col-span-2">
            <label for="ntfy-token" class="label">{m.notify_ntfy_token()}</label>
            <input
              id="ntfy-token"
              type="password"
              class="input"
              bind:value={ntfyToken}
              disabled={ntfyTokenRemove}
              autocomplete="new-password"
              placeholder={ntfyTokenSet ? '••••••••' : 'tk_…'}
            />
            {#if ntfyTokenSet}
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {ntfyTokenRemove ? m.notify_secret_removed() : m.notify_secret_set()}
                <button type="button" class="ml-1 underline" onclick={() => (ntfyTokenRemove = !ntfyTokenRemove)}>
                  {ntfyTokenRemove ? m.common_cancel() : m.notify_secret_remove()}
                </button>
              </p>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <!-- Webhook -->
    <div class="space-y-3 pt-4 border-t border-gray-200 dark:border-gray-700">
      <label class="flex items-start gap-4 cursor-pointer">
        <div class="relative flex items-center">
          <input type="checkbox" bind:checked={webhookEnabled} class="peer sr-only" />
          <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
          <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
        </div>
        <div class="flex-1 min-w-0">
          <div class="font-medium text-gray-900 dark:text-white text-sm">{m.notify_webhook()}</div>
          <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{m.notify_webhook_desc()}</p>
        </div>
      </label>

      {#if webhookEnabled}
        <div class="space-y-3">
          <div>
            <label for="webhook-url" class="label">{m.notify_webhook_url()}</label>
            <input id="webhook-url" type="url" class="input" bind:value={webhookUrl} placeholder="https://…" />
          </div>
          <div>
            <label for="webhook-auth" class="label">{m.notify_webhook_auth()}</label>
            <input
              id="webhook-auth"
              type="password"
              class="input"
              bind:value={webhookAuth}
              disabled={webhookAuthRemove}
              autocomplete="new-password"
              placeholder={webhookAuthSet ? '••••••••' : m.notify_webhook_auth_placeholder()}
            />
            {#if webhookAuthSet}
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {webhookAuthRemove ? m.notify_secret_removed() : m.notify_secret_set()}
                <button type="button" class="ml-1 underline" onclick={() => (webhookAuthRemove = !webhookAuthRemove)}>
                  {webhookAuthRemove ? m.common_cancel() : m.notify_secret_remove()}
                </button>
              </p>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <!-- Link target -->
    <div class="pt-4 border-t border-gray-200 dark:border-gray-700">
      <label for="base-url" class="label">{m.notify_base_url()}</label>
      <div class="flex gap-2">
        <input id="base-url" type="url" class="input flex-1" bind:value={baseUrl} placeholder={window.location.origin} />
        <button type="button" class="btn btn-secondary whitespace-nowrap" onclick={() => (baseUrl = window.location.origin)}>
          {m.notify_base_url_use_current()}
        </button>
      </div>
      <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">{m.notify_base_url_help()}</p>
    </div>

    <!-- Test + save -->
    <div class="pt-4 border-t border-gray-200 dark:border-gray-700 space-y-3">
      {#if testResults}
        <ul class="space-y-1 text-sm" aria-live="polite">
          {#if testResults.length === 0}
            <li class="text-amber-600 dark:text-amber-400">{m.notify_test_none()}</li>
          {/if}
          {#each testResults as result (result.channel)}
            <li class={result.ok ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'}>
              {result.ok
                ? m.notify_test_ok({ channel: result.channel === 'ntfy' ? 'ntfy' : m.notify_webhook() })
                : m.notify_test_failed({ channel: result.channel === 'ntfy' ? 'ntfy' : m.notify_webhook(), error: result.error ?? '' })}
            </li>
          {/each}
        </ul>
      {/if}
      <div class="flex flex-col-reverse sm:flex-row sm:items-center sm:justify-between gap-3">
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="btn btn-secondary"
            onclick={sendTest}
            disabled={testing || (!ntfyEnabled && !webhookEnabled)}
          >
            {testing ? m.notify_testing() : m.notify_test()}
          </button>
          <span class="text-xs text-gray-500 dark:text-gray-400">{m.notify_test_hint()}</span>
        </div>
        <button type="submit" class="btn btn-primary" disabled={saving}>
          {saving ? m.notify_saving() : m.common_save()}
        </button>
      </div>
    </div>
  </form>
{/if}

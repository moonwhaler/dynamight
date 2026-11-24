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
  import Toggle from '../ui/Toggle.svelte';
  import SettingsSection from './SettingsSection.svelte';
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

{#snippet secretField(id: string, label: string, isSet: boolean, remove: boolean, placeholder: string, value: string, setValue: (v: string) => void, toggleRemove: () => void)}
  <div>
    <label for={id} class="label">{label}</label>
    <input
      {id}
      type="password"
      class="input"
      {value}
      oninput={(e) => setValue(e.currentTarget.value)}
      disabled={remove}
      autocomplete="new-password"
      placeholder={isSet ? '••••••••' : placeholder}
    />
    {#if isSet}
      <p class="hint">
        {remove ? m.notify_secret_removed() : m.notify_secret_set()}
        <button type="button" class="ml-1 underline hover:text-gray-700 dark:hover:text-gray-200" onclick={toggleRemove}>
          {remove ? m.common_cancel() : m.notify_secret_remove()}
        </button>
      </p>
    {/if}
  </div>
{/snippet}

{#if loading}
  <div class="flex justify-center py-12 text-primary-600"><Spinner size="w-6 h-6" /></div>
{:else if loadError}
  <div class="card p-4 text-sm text-red-700 dark:text-red-400">
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
    <SettingsSection title={m.notify_events()} description={m.notify_description()}>
      <div class="space-y-5">
        {#each eventOptions as option (option.key)}
          <Toggle label={option.label()} description={option.desc()} bind:checked={events[option.key]} />
        {/each}
      </div>
    </SettingsSection>

    <SettingsSection title={m.notify_channels()}>
      <div class="space-y-6">
        <div class="space-y-4">
          <Toggle label={m.notify_ntfy()} description={m.notify_ntfy_desc()} bind:checked={ntfyEnabled} />
          {#if ntfyEnabled}
            <div class="grid gap-4 sm:grid-cols-2 pl-0 sm:pl-4 sm:border-l-2 border-gray-200 dark:border-gray-700">
              <div>
                <label for="ntfy-server" class="label">{m.notify_ntfy_server()}</label>
                <input id="ntfy-server" type="url" class="input" bind:value={ntfyServer} placeholder="https://ntfy.sh" />
              </div>
              <div>
                <label for="ntfy-topic" class="label">{m.notify_ntfy_topic()}</label>
                <input id="ntfy-topic" type="text" class="input font-mono" bind:value={ntfyTopic} autocomplete="off" />
              </div>
              <p class="sm:col-span-2 hint -mt-2">{m.notify_ntfy_topic_help()}</p>
              <div class="sm:col-span-2">
                {@render secretField('ntfy-token', m.notify_ntfy_token(), ntfyTokenSet, ntfyTokenRemove, 'tk_…', ntfyToken, (v) => (ntfyToken = v), () => (ntfyTokenRemove = !ntfyTokenRemove))}
              </div>
            </div>
          {/if}
        </div>

        <div class="space-y-4 pt-6 border-t border-gray-200 dark:border-gray-700">
          <Toggle label={m.notify_webhook()} description={m.notify_webhook_desc()} bind:checked={webhookEnabled} />
          {#if webhookEnabled}
            <div class="space-y-4 pl-0 sm:pl-4 sm:border-l-2 border-gray-200 dark:border-gray-700">
              <div>
                <label for="webhook-url" class="label">{m.notify_webhook_url()}</label>
                <input id="webhook-url" type="url" class="input" bind:value={webhookUrl} placeholder="https://…" />
              </div>
              {@render secretField('webhook-auth', m.notify_webhook_auth(), webhookAuthSet, webhookAuthRemove, m.notify_webhook_auth_placeholder(), webhookAuth, (v) => (webhookAuth = v), () => (webhookAuthRemove = !webhookAuthRemove))}
            </div>
          {/if}
        </div>
      </div>
    </SettingsSection>

    <SettingsSection title={m.notify_base_url()} description={m.notify_base_url_help()}>
      <div class="flex flex-col sm:flex-row gap-2">
        <input id="base-url" type="url" class="input flex-1" bind:value={baseUrl} placeholder={window.location.origin} aria-label={m.notify_base_url()} />
        <button type="button" class="btn btn-secondary" onclick={() => (baseUrl = window.location.origin)}>
          {m.notify_base_url_use_current()}
        </button>
      </div>
    </SettingsSection>

    <div class="flex flex-col sm:flex-row sm:items-center gap-3">
      <button type="submit" class="btn btn-primary" disabled={saving}>
        {saving ? m.notify_saving() : m.common_save()}
      </button>
      <button type="button" class="btn btn-secondary" onclick={sendTest} disabled={testing || (!ntfyEnabled && !webhookEnabled)}>
        {testing ? m.notify_testing() : m.notify_test()}
      </button>
      <span class="text-xs text-gray-500 dark:text-gray-400">{m.notify_test_hint()}</span>
    </div>

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
  </form>
{/if}

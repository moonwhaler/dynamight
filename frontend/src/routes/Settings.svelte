<script lang="ts">
  import { push } from 'svelte-spa-router';
  import PageHeader from '../components/ui/PageHeader.svelte';
  import Icon, { type IconName } from '../components/ui/Icon.svelte';
  import SettingsGeneral from '../components/settings/SettingsGeneral.svelte';
  import SettingsAppearance from '../components/settings/SettingsAppearance.svelte';
  import SettingsAccount from '../components/settings/SettingsAccount.svelte';
  import SettingsSecurity from '../components/settings/SettingsSecurity.svelte';
  import CredentialsManager from '../components/settings/CredentialsManager.svelte';
  import NotificationSettings from '../components/settings/NotificationSettings.svelte';
  import SettingsData from '../components/settings/SettingsData.svelte';
  import SettingsAbout from '../components/settings/SettingsAbout.svelte';
  import { authStore } from '../lib/stores/auth';
  import * as m from '$lib/paraglide/messages.js';

  let { params = {} }: { params?: { section?: string } } = $props();

  const SECTIONS: { id: string; label: () => string; desc: () => string; icon: IconName }[] = [
    { id: 'general', label: m.settings_tab_general, desc: m.settings_tab_general_desc, icon: 'sliders' },
    { id: 'appearance', label: m.settings_tab_appearance, desc: m.settings_tab_appearance_desc, icon: 'swatch' },
    { id: 'notifications', label: m.settings_tab_notifications, desc: m.settings_tab_notifications_desc, icon: 'bell' },
    { id: 'credentials', label: m.settings_tab_credentials, desc: m.settings_tab_credentials_desc, icon: 'key' },
    { id: 'account', label: m.settings_tab_account, desc: m.settings_tab_account_desc, icon: 'user' },
    { id: 'security', label: m.settings_tab_security, desc: m.settings_tab_security_desc, icon: 'shield' },
    { id: 'data', label: m.settings_tab_data, desc: m.settings_tab_data_desc, icon: 'database' },
    { id: 'about', label: m.settings_tab_about, desc: m.settings_tab_about_desc, icon: 'info' },
  ];

  // Phones show the list on /settings and one section per page; wider
  // screens show the list next to the section (general by default).
  let wide = $state(typeof window !== 'undefined' && window.matchMedia('(min-width: 768px)').matches);
  $effect(() => {
    const mq = window.matchMedia('(min-width: 768px)');
    const update = () => (wide = mq.matches);
    mq.addEventListener('change', update);
    return () => mq.removeEventListener('change', update);
  });

  const requested = $derived(SECTIONS.find((s) => s.id === params.section) ?? null);
  const current = $derived(requested ?? (wide ? SECTIONS[0] : null));

  $effect(() => {
    // Unknown section names fall back to the list
    if (params.section && !requested) push('/settings');
  });
</script>

{#if current && !wide}
  <a href="#/settings" class="inline-flex items-center gap-1.5 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white mb-3">
    <Icon name="chevron-left" class="w-4 h-4" />
    {m.common_settings()}
  </a>
{/if}

<PageHeader title={current && !wide ? current.label() : m.common_settings()} subtitle={current && !wide ? current.desc() : undefined} />

<div class="md:grid md:grid-cols-[13rem_1fr] lg:grid-cols-[15rem_1fr] md:gap-8 lg:gap-10 items-start">
  {#if wide || !current}
    <nav aria-label={m.common_settings()} class="md:sticky md:top-8">
      <ul class="card md:border-0 md:bg-transparent md:dark:bg-transparent rows md:[&>*+*]:border-0 overflow-hidden">
        {#each SECTIONS as section (section.id)}
          {@const active = current?.id === section.id}
          <li>
            <a
              href="#/settings/{section.id}"
              aria-current={active ? 'page' : undefined}
              class="flex items-center gap-3 px-4 md:px-3 py-3.5 md:py-2 md:rounded-lg transition-colors
                {active
                ? 'md:bg-gray-200/70 md:dark:bg-gray-800 text-gray-900 dark:text-white'
                : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800/60'}"
            >
              <Icon name={section.icon} class="w-5 h-5 text-gray-500 dark:text-gray-400 shrink-0" />
              <span class="flex-1 min-w-0">
                <span class="block text-sm font-medium">{section.label()}</span>
                <span class="block md:hidden text-xs text-gray-500 dark:text-gray-400 truncate">{section.desc()}</span>
              </span>
              <Icon name="chevron-right" class="w-4 h-4 text-gray-400 md:hidden" />
            </a>
          </li>
        {/each}
      </ul>
      {#if !wide}
        <p class="text-center text-xs text-gray-500 dark:text-gray-400 mt-6">
          {m.settings_signed_in_as({ name: $authStore.user?.username ?? '' })}
          · <button type="button" class="underline" onclick={() => authStore.logout()}>{m.auth_logout()}</button>
        </p>
      {/if}
    </nav>
  {/if}

  {#if current}
    <div class="min-w-0 max-w-3xl">
      {#if current.id === 'general'}<SettingsGeneral />
      {:else if current.id === 'appearance'}<SettingsAppearance />
      {:else if current.id === 'notifications'}<NotificationSettings />
      {:else if current.id === 'credentials'}<CredentialsManager />
      {:else if current.id === 'account'}<SettingsAccount />
      {:else if current.id === 'security'}<SettingsSecurity />
      {:else if current.id === 'data'}<SettingsData />
      {:else if current.id === 'about'}<SettingsAbout />
      {/if}
    </div>
  {/if}
</div>

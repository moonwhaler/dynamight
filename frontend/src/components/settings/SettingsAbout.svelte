<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../../lib/api';
  import SettingsSection from './SettingsSection.svelte';
  import Icon from '../ui/Icon.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let info = $state<{ version: string; build: string; date: string } | null>(null);

  onMount(async () => {
    try {
      info = await api.system.version();
    } catch {
      // Version endpoint unavailable: leave the fields empty
    }
  });
</script>

<SettingsSection title="Dynamight" description={m.about_tagline()}>
  <dl class="grid grid-cols-[auto_1fr] gap-x-8 gap-y-3 text-sm">
    <dt class="text-gray-500 dark:text-gray-400">{m.about_version()}</dt>
    <dd class="font-mono text-gray-900 dark:text-white">
      {info?.version ?? '—'}
      {#if info?.build}<span class="text-gray-400"> ({info.build}{info.date ? `, ${info.date}` : ''})</span>{/if}
    </dd>
    <dt class="text-gray-500 dark:text-gray-400">{m.about_license()}</dt>
    <dd class="text-gray-900 dark:text-white">MIT</dd>
    <dt class="text-gray-500 dark:text-gray-400">{m.about_author()}</dt>
    <dd class="text-gray-900 dark:text-white">moonwhaler</dd>
  </dl>
  <a
    href="https://github.com/moonwhaler/dynamight"
    target="_blank"
    rel="noopener noreferrer"
    class="btn btn-secondary mt-6"
  >
    {m.about_view_on_github()}
    <Icon name="external" class="w-4 h-4" />
  </a>
</SettingsSection>

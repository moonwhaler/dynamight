<script lang="ts">
  import type { S3DestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    config = $bindable<S3DestinationConfig>(),
    credentialId = $bindable<number | null>(null),
    credentials = [],
  }: {
    config: S3DestinationConfig;
    credentialId: number | null;
    credentials: Credential[];
  } = $props();

  const regions = [
    { value: 'us-east-1', label: 'US East (N. Virginia)' },
    { value: 'us-east-2', label: 'US East (Ohio)' },
    { value: 'us-west-1', label: 'US West (N. California)' },
    { value: 'us-west-2', label: 'US West (Oregon)' },
    { value: 'eu-west-1', label: 'EU (Ireland)' },
    { value: 'eu-west-2', label: 'EU (London)' },
    { value: 'eu-central-1', label: 'EU (Frankfurt)' },
    { value: 'ap-southeast-1', label: 'Asia Pacific (Singapore)' },
    { value: 'ap-southeast-2', label: 'Asia Pacific (Sydney)' },
    { value: 'ap-northeast-1', label: 'Asia Pacific (Tokyo)' },
    { value: 'custom', labelKey: () => m.s3_region_custom() },
  ];

  const storageClasses = [
    { value: '', labelKey: () => m.s3_storage_class_default() },
    { value: 'STANDARD', labelKey: () => m.s3_storage_class_standard() },
    { value: 'STANDARD_IA', labelKey: () => m.s3_storage_class_standard_ia() },
    { value: 'ONEZONE_IA', labelKey: () => m.s3_storage_class_onezone_ia() },
    { value: 'GLACIER', labelKey: () => m.s3_storage_class_glacier() },
    { value: 'GLACIER_IR', labelKey: () => m.s3_storage_class_glacier_ir() },
    { value: 'DEEP_ARCHIVE', labelKey: () => m.s3_storage_class_deep_archive() },
  ];

  let isCustomEndpoint = $derived(config.region === 'custom' || !!config.endpoint);
</script>

<div class="space-y-4">
  <CredentialSelector
    providerType="s3"
    bind:selected={credentialId}
    {credentials}
  />

  <div>
    <label for="bucket" class="label">
      {m.s3_bucket()}
      <HelpTooltip text={m.s3_bucket_help()} />
    </label>
    <input
      type="text"
      id="bucket"
      bind:value={config.bucket}
      placeholder={m.s3_bucket_placeholder()}
      class="input"
    />
  </div>

  <div>
    <label for="region" class="label">
      {m.s3_region()}
      <HelpTooltip text={m.s3_region_help()} />
    </label>
    <select id="region" bind:value={config.region} class="input">
      {#each regions as region}
        <option value={region.value}>{region.labelKey ? region.labelKey() : region.label}</option>
      {/each}
    </select>
  </div>

  {#if isCustomEndpoint}
    <div>
      <label for="endpoint" class="label">
        {m.s3_custom_endpoint_url()}
        <HelpTooltip text={m.s3_custom_endpoint_help()} />
      </label>
      <input
        type="url"
        id="endpoint"
        bind:value={config.endpoint}
        placeholder={m.s3_custom_endpoint_placeholder()}
        class="input"
      />
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
        {m.s3_custom_endpoint_note()}
      </p>
    </div>
  {/if}

  <div>
    <label for="prefix" class="label">
      {m.s3_path_prefix()}
      <HelpTooltip text={m.s3_path_prefix_help()} />
    </label>
    <input
      type="text"
      id="prefix"
      bind:value={config.prefix}
      placeholder={m.s3_path_prefix_placeholder()}
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      {m.s3_preview({ bucket: config.bucket || '', prefix: config.prefix || '' })}
    </p>
  </div>

  <div>
    <label for="storage-class" class="label">
      {m.s3_storage_class()}
      <HelpTooltip text={m.s3_storage_class_help()} />
    </label>
    <select id="storage-class" bind:value={config.storage_class} class="input">
      {#each storageClasses as sc}
        <option value={sc.value || null}>{sc.labelKey()}</option>
      {/each}
    </select>
  </div>
</div>

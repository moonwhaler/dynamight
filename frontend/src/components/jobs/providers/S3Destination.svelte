<script lang="ts">
  import type { S3DestinationConfig, Credential } from '../../../lib/types';
  import CredentialSelector from '../CredentialSelector.svelte';
  import HelpTooltip from '../../ui/HelpTooltip.svelte';

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
    { value: 'custom', label: 'Custom Endpoint' },
  ];

  const storageClasses = [
    { value: '', label: 'Default' },
    { value: 'STANDARD', label: 'Standard' },
    { value: 'STANDARD_IA', label: 'Standard-IA (Infrequent Access)' },
    { value: 'ONEZONE_IA', label: 'One Zone-IA' },
    { value: 'GLACIER', label: 'Glacier' },
    { value: 'GLACIER_IR', label: 'Glacier Instant Retrieval' },
    { value: 'DEEP_ARCHIVE', label: 'Glacier Deep Archive' },
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
      Bucket Name
      <HelpTooltip text="The name of your S3 bucket where backups will be stored." />
    </label>
    <input
      type="text"
      id="bucket"
      bind:value={config.bucket}
      placeholder="my-backup-bucket"
      class="input"
    />
  </div>

  <div>
    <label for="region" class="label">
      Region
      <HelpTooltip text="The AWS region where your bucket is located. Select 'Custom Endpoint' for S3-compatible services like MinIO or Backblaze B2." />
    </label>
    <select id="region" bind:value={config.region} class="input">
      {#each regions as region}
        <option value={region.value}>{region.label}</option>
      {/each}
    </select>
  </div>

  {#if isCustomEndpoint}
    <div>
      <label for="endpoint" class="label">
        Custom Endpoint URL
        <HelpTooltip text="The URL for S3-compatible services like MinIO, Backblaze B2, or Wasabi." />
      </label>
      <input
        type="url"
        id="endpoint"
        bind:value={config.endpoint}
        placeholder="https://s3.example.com"
        class="input"
      />
      <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
        For MinIO, Backblaze B2, Wasabi, or other S3-compatible services.
      </p>
    </div>
  {/if}

  <div>
    <label for="prefix" class="label">
      Path Prefix
      <HelpTooltip text="A prefix (folder path) within the bucket where backups will be stored." />
    </label>
    <input
      type="text"
      id="prefix"
      bind:value={config.prefix}
      placeholder="backups/"
      class="input"
    />
    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Files will be stored at: s3://{config.bucket}/{config.prefix}
    </p>
  </div>

  <div>
    <label for="storage-class" class="label">
      Storage Class
      <HelpTooltip text="The S3 storage class to use. Different classes have different pricing and retrieval times." />
    </label>
    <select id="storage-class" bind:value={config.storage_class} class="input">
      {#each storageClasses as sc}
        <option value={sc.value || null}>{sc.label}</option>
      {/each}
    </select>
  </div>
</div>

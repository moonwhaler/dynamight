<script lang="ts">
  // Destination type and its settings, bound to a job draft.
  import type { JobDraft } from '$lib/jobDraft.svelte';
  import type { Credential, UsbDrive } from '$lib/types';
  import ProviderSelector from '../ProviderSelector.svelte';
  import LocalDestination from '../providers/LocalDestination.svelte';
  import S3Destination from '../providers/S3Destination.svelte';
  import SftpDestination from '../providers/SftpDestination.svelte';
  import WebDavDestination from '../providers/WebDavDestination.svelte';
  import GoogleDriveDestination from '../providers/GoogleDriveDestination.svelte';
  import OneDriveDestination from '../providers/OneDriveDestination.svelte';
  import Icon from '../../ui/Icon.svelte';
  import { DESTINATION_ICON, destinationKindLabel } from '$lib/utils/destination';
  import { destinationKind } from '$lib/utils/jobUtils';
  import * as m from '$lib/paraglide/messages.js';

  let {
    draft,
    drives,
    credentials,
    onCredentialsChange,
    compact = false,
  }: {
    draft: JobDraft;
    drives: UsbDrive[];
    credentials: Credential[];
    onCredentialsChange: () => void;
    /** Existing jobs: show the type as one row with "Change" instead of all choices */
    compact?: boolean;
  } = $props();

  let choosing = $state(false);
  const kind = $derived(destinationKind({ destination: draft.destination, destination_type: draft.destinationType }));
  const forType = (t: string) => credentials.filter((c) => c.provider_type === t);
</script>

<div class="space-y-6">
  {#if compact && !choosing}
    <div class="flex items-center justify-between gap-3 p-3 rounded-lg bg-gray-100 dark:bg-gray-900/50">
      <span class="flex items-center gap-2.5 text-sm font-medium text-gray-900 dark:text-white">
        <Icon name={DESTINATION_ICON[kind]} class="w-5 h-5 text-gray-500" />
        {destinationKindLabel(kind)}
      </span>
      <button type="button" class="btn btn-ghost btn-sm" onclick={() => (choosing = true)}>{m.editor_change_type()}</button>
    </div>
  {:else}
    <ProviderSelector bind:selected={() => draft.destinationType, (v) => draft.setDestinationType(v)} />
  {/if}

  {#if draft.destinationType === 'local' && draft.destination.type === 'local'}
    <LocalDestination bind:config={draft.destination} {drives} />
  {:else if draft.destinationType === 's3' && draft.destination.type === 's3'}
    <S3Destination bind:config={draft.destination} bind:credentialId={draft.credentialId} credentials={forType('s3')} {onCredentialsChange} />
  {:else if draft.destinationType === 'sftp' && draft.destination.type === 'sftp'}
    <SftpDestination bind:config={draft.destination} bind:credentialId={draft.credentialId} credentials={forType('sftp')} {onCredentialsChange} />
  {:else if draft.destinationType === 'webdav' && draft.destination.type === 'webdav'}
    <WebDavDestination bind:config={draft.destination} bind:credentialId={draft.credentialId} credentials={forType('webdav')} {onCredentialsChange} />
  {:else if draft.destinationType === 'google_drive' && draft.destination.type === 'google_drive'}
    <GoogleDriveDestination bind:config={draft.destination} bind:credentialId={draft.credentialId} credentials={forType('google_drive')} {onCredentialsChange} />
  {:else if draft.destinationType === 'onedrive' && draft.destination.type === 'onedrive'}
    <OneDriveDestination bind:config={draft.destination} bind:credentialId={draft.credentialId} credentials={forType('onedrive')} {onCredentialsChange} />
  {/if}
</div>

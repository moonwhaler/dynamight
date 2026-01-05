<script lang="ts">
  import { onMount } from 'svelte';
  import { push } from 'svelte-spa-router';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import { preferencesStore } from '../lib/stores/preferences';
  import type {
    Job,
    Schedule,
    UsbDrive,
    CreateJobRequest,
    DestinationType,
    DestinationConfig,
    SyncOptions,
    Credential,
    ProviderCapabilities,
  } from '../lib/types';
  import { createDefaultDestination, createDefaultSyncOptions } from '../lib/types';
  import ProviderSelector from '../components/jobs/ProviderSelector.svelte';
  import SyncOptionsComponent from '../components/jobs/SyncOptions.svelte';
  import SchedulePicker from '../components/jobs/SchedulePicker.svelte';
  import PathSelector from '../components/jobs/PathSelector.svelte';
  import HelpTooltip from '../components/ui/HelpTooltip.svelte';
  import RunLogModal from '../components/logs/RunLogModal.svelte';
  import { confirm } from '../components/ui/ConfirmDialog.svelte';
  import { showToast } from '../components/ui/Toast.svelte';

  // Provider destination components
  import GoogleDriveDestination from '../components/jobs/providers/GoogleDriveDestination.svelte';
  import LocalDestination from '../components/jobs/providers/LocalDestination.svelte';
  import OneDriveDestination from '../components/jobs/providers/OneDriveDestination.svelte';
  import S3Destination from '../components/jobs/providers/S3Destination.svelte';
  import SftpDestination from '../components/jobs/providers/SftpDestination.svelte';
  import WebDavDestination from '../components/jobs/providers/WebDavDestination.svelte';

  let { params = {} }: { params?: { id?: string } } = $props();

  let isNew = $derived(!params.id || params.id === 'new');
  let loading = $state(true);
  let saving = $state(false);
  let drives = $state<UsbDrive[]>([]);
  let credentials = $state<Credential[]>([]);
  let schedules = $state<Schedule[]>([]);
  let loadedJobId = $state<string | null>(null);
  let activeRunId = $state<number | null>(null);
  let running = $state(false);
  let cloning = $state(false);
  let capabilities = $state<ProviderCapabilities | null>(null);

  // Form state
  let name = $state('');
  let description = $state('');
  let enabled = $state(true);
  let sourceDirs = $state<string[]>([]);

  // Provider-based state
  let destinationType = $state<DestinationType>('local');
  let destination = $state<DestinationConfig>(createDefaultDestination('local'));
  let syncOptions = $state<SyncOptions>(createDefaultSyncOptions());
  let credentialId = $state<number | null>(null);

  async function loadData() {
    loading = true;

    try {
      // Load USB drives and credentials in parallel
      const [drivesResult, credentialsResult] = await Promise.all([
        api.system.drives(),
        api.credentials.list(),
      ]);
      drives = drivesResult;
      credentials = credentialsResult;

      if (!isNew && params.id && params.id !== loadedJobId) {
        loadedJobId = params.id;
        const job = await api.jobs.get(parseInt(params.id));
        loadJob(job);
        schedules = await api.schedules.list(job.id);
      }

      // Load capabilities for the current provider
      await loadCapabilities(destinationType);
    } catch (e) {
      console.error('[JobDetail] Error:', e);
      showToast({ message: e instanceof Error ? e.message : 'Failed to load job', variant: 'error' });
    } finally {
      loading = false;
    }
  }

  async function loadCapabilities(type: DestinationType) {
    try {
      capabilities = await api.providers.capabilities(type);
    } catch {
      capabilities = null;
    }
  }

  async function loadCredentials() {
    try {
      credentials = await api.credentials.list();
    } catch (e) {
      console.error('Failed to reload credentials:', e);
    }
  }

  onMount(() => {
    loadData();
  });

  // Reload when params change
  $effect(() => {
    const currentId = params.id;
    if (currentId && currentId !== 'new' && currentId !== loadedJobId) {
      loadData();
    }
  });

  // Update destination config when type changes
  $effect(() => {
    const type = destinationType;
    // Only reset if the type doesn't match
    if (destination.type !== type) {
      destination = createDefaultDestination(type);
      credentialId = null;
    }
    loadCapabilities(type);
  });

  function loadJob(job: Job) {
    name = job.name;
    description = job.description || '';
    enabled = job.enabled;
    sourceDirs = job.source_dirs;

    // Load provider-based fields
    destinationType = job.destination_type || 'local';
    destination = job.destination || createDefaultDestination(destinationType);
    syncOptions = job.sync_options || createDefaultSyncOptions();
    credentialId = job.credential_id;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();

    // Validate at least one source directory is selected
    if (sourceDirs.length === 0) {
      showToast({ message: 'At least one source directory must be selected', variant: 'error' });
      return;
    }

    // Validate credentials for providers that require them
    if (destinationType !== 'local' && !credentialId) {
      showToast({ message: 'Please select credentials for this provider', variant: 'error' });
      return;
    }

    // Warn about Mirror Mode
    if (syncOptions.delete_extraneous) {
      const confirmed = await confirm({
        title: 'Mirror Mode Enabled',
        message:
          'Mirror Mode will delete files from the backup destination that no longer exist in the source. This can result in permanent data loss if files are accidentally deleted from the source. Are you sure you want to continue?',
        confirmText: 'Yes, enable Mirror Mode',
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    saving = true;

    // Build job data with both legacy and new formats for compatibility
    const jobData: CreateJobRequest = {
      name,
      description: description || undefined,
      enabled,
      source_dirs: sourceDirs,

      // New provider-based fields
      destination_type: destinationType,
      destination,
      sync_options: syncOptions,
      credential_id: credentialId || undefined,

      // Legacy fields for local destinations (backwards compatibility)
      ...(destinationType === 'local' && destination.type === 'local'
        ? {
            mount_point: destination.mount_point,
            backup_subdir: destination.backup_subdir,
            usb_uuid: destination.usb_uuid || undefined,
            auto_mount: destination.auto_mount,
            auto_unmount: destination.auto_unmount,
            sync_deletes: syncOptions.delete_extraneous,
            rsync_excludes: syncOptions.exclude_patterns.length > 0 ? syncOptions.exclude_patterns : undefined,
            checksum_mode: (syncOptions.provider_options?.checksum_mode as boolean) || false,
            compress: (syncOptions.provider_options?.compress as boolean) || false,
            dry_run: syncOptions.dry_run,
            bandwidth_limit: syncOptions.bandwidth_limit_kbps || undefined,
            verbosity: syncOptions.verbosity,
          }
        : {}),
    };

    try {
      if (isNew) {
        const job = await api.jobs.create(jobData);
        jobsStore.addJob(job);
        showToast({ message: 'Job created successfully', variant: 'success' });
      } else {
        const job = await api.jobs.update(parseInt(params.id!), jobData);
        jobsStore.updateJob(job);
        showToast({ message: 'Job saved successfully', variant: 'success' });
      }
      push('/jobs');
    } catch (e) {
      console.error('[JobDetail] Submit error:', e);
      showToast({ message: e instanceof Error ? e.message : 'Failed to save job', variant: 'error' });
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    const confirmed = await confirm({
      title: 'Delete Job',
      message: `Are you sure you want to delete "${name}"? This action cannot be undone.`,
      confirmText: 'Delete',
      variant: 'danger',
    });
    if (!confirmed) return;

    try {
      await api.jobs.delete(parseInt(params.id!));
      jobsStore.removeJob(parseInt(params.id!));
      push('/jobs');
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : 'Failed to delete job', variant: 'error' });
    }
  }

  async function handleRun() {
    if (running) return;
    running = true;
    try {
      const result = await api.jobs.run(parseInt(params.id!));
      if ($preferencesStore.showLogViewerAfterManualRun) {
        activeRunId = result.runId;
      }
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : 'Failed to start job', variant: 'error' });
    } finally {
      running = false;
    }
  }

  function closeRunModal() {
    activeRunId = null;
  }

  async function handleClone() {
    if (cloning) return;
    cloning = true;
    try {
      const clonedJob = await api.jobs.clone(parseInt(params.id!));
      jobsStore.addJob(clonedJob);
      push(`/jobs/${clonedJob.id}`);
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : 'Failed to clone job', variant: 'error' });
    } finally {
      cloning = false;
    }
  }
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">
      {isNew ? 'New Backup Job' : 'Edit Backup Job'}
    </h1>
    {#if !isNew}
      <div class="flex gap-2">
        <button onclick={handleRun} disabled={running} class="btn btn-secondary flex-1 sm:flex-none">
          {running ? 'Starting...' : 'Run Now'}
        </button>
        <button onclick={handleClone} disabled={cloning} class="btn btn-secondary flex-1 sm:flex-none">
          {cloning ? 'Cloning...' : 'Clone'}
        </button>
        <button onclick={handleDelete} class="btn btn-danger flex-1 sm:flex-none">Delete</button>
      </div>
    {/if}
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else}
    <!-- Job Status Toggle -->
    <label
      class="card flex items-center justify-between gap-4 p-4 cursor-pointer group transition-colors {enabled
        ? 'ring-1 ring-primary-500/50 bg-primary-50/30 dark:bg-primary-900/10'
        : 'bg-gray-50 dark:bg-gray-800/50'}"
    >
      <div class="flex items-center gap-3">
        <div
          class="w-10 h-10 rounded-xl flex items-center justify-center transition-colors {enabled
            ? 'bg-primary-100 dark:bg-primary-900/40'
            : 'bg-gray-200 dark:bg-gray-700'}"
        >
          {#if enabled}
            <svg class="w-5 h-5 text-primary-600 dark:text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          {:else}
            <svg class="w-5 h-5 text-gray-400 dark:text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          {/if}
        </div>
        <div>
          <div class="font-medium text-gray-900 dark:text-white">
            {enabled ? 'Job Enabled' : 'Job Disabled'}
          </div>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {enabled ? 'This job will run on schedule' : 'Scheduled runs are paused'}
          </p>
        </div>
      </div>
      <div class="relative flex items-center">
        <input type="checkbox" bind:checked={enabled} class="peer sr-only" />
        <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
        <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
      </div>
    </label>

    <form onsubmit={handleSubmit} class="space-y-6">
      <!-- Basic Info -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Basic Information</h2>

        <div>
          <label for="name" class="label">Job Name</label>
          <input type="text" id="name" bind:value={name} required class="input" />
        </div>

        <div>
          <label for="description" class="label">Description</label>
          <textarea id="description" bind:value={description} rows="2" class="input"></textarea>
        </div>
      </div>

      <!-- Destination Type Selection -->
      <div class="card p-6">
        <ProviderSelector bind:selected={destinationType} />
      </div>

      <!-- Provider-specific Destination Configuration -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
          {#if destinationType === 'local'}
            Mount Configuration
          {:else if destinationType === 's3'}
            S3 Configuration
          {:else if destinationType === 'sftp'}
            SFTP Configuration
          {:else if destinationType === 'webdav'}
            WebDAV Configuration
          {:else}
            Destination Configuration
          {/if}
        </h2>

        {#if destinationType === 'local' && destination.type === 'local'}
          <LocalDestination bind:config={destination} {drives} />
        {:else if destinationType === 's3' && destination.type === 's3'}
          <S3Destination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 's3')}
          />
        {:else if destinationType === 'sftp' && destination.type === 'sftp'}
          <SftpDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'sftp')}
          />
        {:else if destinationType === 'webdav' && destination.type === 'webdav'}
          <WebDavDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'webdav')}
          />
        {:else if destinationType === 'google_drive' && destination.type === 'google_drive'}
          <GoogleDriveDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'google_drive')}
          />
        {:else if destinationType === 'onedrive' && destination.type === 'onedrive'}
          <OneDriveDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'onedrive')}
          />
        {:else}
          <p class="text-gray-500 dark:text-gray-400">
            This destination type is not yet supported.
          </p>
        {/if}
      </div>

      <!-- Source Directories -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
          Source Directories
          <HelpTooltip
            text="The folders on your system that you want to back up. You can add multiple directories, and each will be synced to a matching subfolder in the destination."
          />
        </h2>
        <PathSelector bind:paths={sourceDirs} />
      </div>

      <!-- Sync Options -->
      <div class="card p-6">
        <SyncOptionsComponent bind:options={syncOptions} {destinationType} {capabilities} />
      </div>

      <!-- Schedule -->
      {#if !isNew}
        <div class="card p-6">
          <SchedulePicker jobId={parseInt(params.id!)} bind:schedules />
        </div>
      {/if}

      <!-- Actions -->
      <div class="flex flex-col-reverse sm:flex-row sm:justify-end gap-3">
        <a href="#/jobs" class="btn btn-secondary text-center">Cancel</a>
        <button type="submit" disabled={saving} class="btn btn-primary">
          {saving ? (isNew ? 'Adding...' : 'Saving...') : isNew ? 'Add Job' : 'Save Changes'}
        </button>
      </div>
    </form>
  {/if}
</div>

<!-- Run Log Modal -->
{#if activeRunId !== null}
  <RunLogModal runId={activeRunId} jobId={parseInt(params.id!)} onClose={closeRunModal} />
{/if}

<script lang="ts">
  import { onMount } from 'svelte';
  import { push } from 'svelte-spa-router';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import { preferencesStore } from '../lib/stores/preferences';
  import * as m from '$lib/paraglide/messages.js';
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
  let checkingSpace = $state(false);
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

  // Pending schedules for new jobs (cron expressions stored until job is created)
  let pendingSchedules = $state<string[]>([]);

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
      showToast({ message: e instanceof Error ? e.message : m.error_failed_load(), variant: 'error' });
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
      showToast({ message: m.job_validation_source_required(), variant: 'error' });
      return;
    }

    // Validate credentials for providers that require them
    if (destinationType !== 'local' && !credentialId) {
      showToast({ message: m.job_validation_credentials_required(), variant: 'error' });
      return;
    }

    // Warn about Mirror Mode
    if (syncOptions.delete_extraneous) {
      const confirmed = await confirm({
        title: m.job_confirm_mirror_title(),
        message: m.job_confirm_mirror_message(),
        confirmText: m.job_confirm_mirror_button(),
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

        // Create pending schedules for the new job
        if (pendingSchedules.length > 0) {
          for (const cron of pendingSchedules) {
            try {
              await api.schedules.create(job.id, { cron_expression: cron });
            } catch (e) {
              console.error('[JobDetail] Failed to create schedule:', e);
            }
          }
        }

        showToast({ message: m.job_toast_created(), variant: 'success' });
      } else {
        const job = await api.jobs.update(parseInt(params.id!), jobData);
        jobsStore.updateJob(job);
        showToast({ message: m.job_toast_saved(), variant: 'success' });
      }
      push('/jobs');
    } catch (e) {
      console.error('[JobDetail] Submit error:', e);
      showToast({ message: e instanceof Error ? e.message : m.job_error_save(), variant: 'error' });
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    const confirmed = await confirm({
      title: m.job_confirm_delete_title(),
      message: m.job_confirm_delete_message({ name }),
      confirmText: m.common_delete(),
      variant: 'danger',
    });
    if (!confirmed) return;

    try {
      await api.jobs.delete(parseInt(params.id!));
      jobsStore.removeJob(parseInt(params.id!));
      push('/jobs');
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.job_error_delete(), variant: 'error' });
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
      showToast({ message: e instanceof Error ? e.message : m.job_error_start(), variant: 'error' });
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
      showToast({ message: e instanceof Error ? e.message : m.job_error_clone(), variant: 'error' });
    } finally {
      cloning = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  async function handleCheckSpace() {
    if (checkingSpace || isNew) return;
    checkingSpace = true;
    try {
      const result = await api.jobs.checkSpace(parseInt(params.id!));
      if (result.fits) {
        showToast({
          message: `${m.space_check_fits()}: ${formatBytes(result.transfer_size)} to transfer, ${formatBytes(result.destination_free)} free`,
          variant: 'success',
        });
      } else {
        showToast({
          message: `${m.space_check_insufficient()}: ${m.space_check_deficit()} ${formatBytes(result.deficit ?? 0)}`,
          variant: 'error',
        });
      }
    } catch (e) {
      showToast({
        message: m.space_check_error(),
        variant: 'error',
      });
    } finally {
      checkingSpace = false;
    }
  }
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">
      {isNew ? m.jobs_title_new() : m.jobs_title_edit()}
    </h1>
    {#if !isNew}
      <div class="flex gap-2">
        {#if destinationType === 'local'}
          <button onclick={handleCheckSpace} disabled={checkingSpace} class="btn btn-secondary flex-1 sm:flex-none">
            {checkingSpace ? m.space_check_checking() : m.space_check_button()}
          </button>
        {/if}
        <button onclick={handleRun} disabled={running} class="btn btn-secondary flex-1 sm:flex-none">
          {running ? m.job_btn_starting() : m.job_btn_run_now()}
        </button>
        <button onclick={handleClone} disabled={cloning} class="btn btn-secondary flex-1 sm:flex-none">
          {cloning ? m.job_btn_cloning() : m.common_clone()}
        </button>
        <button onclick={handleDelete} class="btn btn-danger flex-1 sm:flex-none">{m.common_delete()}</button>
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
            {enabled ? m.job_status_enabled() : m.job_status_disabled()}
          </div>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {enabled ? m.job_status_enabled_desc() : m.job_status_disabled_desc()}
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
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{m.job_section_basic()}</h2>

        <div>
          <label for="name" class="label">{m.job_label_name()}</label>
          <input type="text" id="name" bind:value={name} required class="input" />
        </div>

        <div>
          <label for="description" class="label">{m.job_label_description()}</label>
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
            {m.provider_config_local()}
          {:else if destinationType === 's3'}
            {m.provider_config_s3()}
          {:else if destinationType === 'sftp'}
            {m.provider_config_sftp()}
          {:else if destinationType === 'webdav'}
            {m.provider_config_webdav()}
          {:else if destinationType === 'google_drive'}
            {m.provider_config_google_drive()}
          {:else if destinationType === 'onedrive'}
            {m.provider_config_onedrive()}
          {:else}
            {m.provider_config_generic()}
          {/if}
        </h2>

        {#if destinationType === 'local' && destination.type === 'local'}
          <LocalDestination bind:config={destination} {drives} />
        {:else if destinationType === 's3' && destination.type === 's3'}
          <S3Destination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 's3')}
            onCredentialsChange={loadCredentials}
          />
        {:else if destinationType === 'sftp' && destination.type === 'sftp'}
          <SftpDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'sftp')}
            onCredentialsChange={loadCredentials}
          />
        {:else if destinationType === 'webdav' && destination.type === 'webdav'}
          <WebDavDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'webdav')}
            onCredentialsChange={loadCredentials}
          />
        {:else if destinationType === 'google_drive' && destination.type === 'google_drive'}
          <GoogleDriveDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'google_drive')}
            onCredentialsChange={loadCredentials}
          />
        {:else if destinationType === 'onedrive' && destination.type === 'onedrive'}
          <OneDriveDestination
            bind:config={destination}
            bind:credentialId
            credentials={credentials.filter((c) => c.provider_type === 'onedrive')}
            onCredentialsChange={loadCredentials}
          />
        {:else}
          <p class="text-gray-500 dark:text-gray-400">
            {m.job_destination_not_supported()}
          </p>
        {/if}
      </div>

      <!-- Source Directories -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
          {m.job_section_sources()}
          <HelpTooltip
            text={m.job_sources_help()}
          />
        </h2>
        <PathSelector bind:paths={sourceDirs} />
      </div>

      <!-- Sync Options -->
      <div class="card p-6">
        <SyncOptionsComponent bind:options={syncOptions} {destinationType} {capabilities} {sourceDirs} />
      </div>

      <!-- Schedule -->
      <div class="card p-6">
        {#if isNew}
          <SchedulePicker bind:schedules bind:pendingSchedules />
        {:else}
          <SchedulePicker jobId={parseInt(params.id!)} bind:schedules />
        {/if}
      </div>

      <!-- Actions -->
      <div class="flex flex-col-reverse sm:flex-row sm:justify-end gap-3">
        <a href="#/jobs" class="btn btn-secondary text-center">{m.common_cancel()}</a>
        <button type="submit" disabled={saving} class="btn btn-primary">
          {saving ? (isNew ? m.job_btn_adding() : m.job_btn_saving()) : isNew ? m.job_btn_add() : m.job_btn_save()}
        </button>
      </div>
    </form>
  {/if}
</div>

<!-- Run Log Modal -->
{#if activeRunId !== null}
  <RunLogModal runId={activeRunId} jobId={parseInt(params.id!)} onClose={closeRunModal} />
{/if}

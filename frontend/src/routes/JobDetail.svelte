<script lang="ts">
  import { onMount } from 'svelte';
  import { push } from 'svelte-spa-router';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import { preferencesStore } from '../lib/stores/preferences';
  import type { Job, Schedule, UsbDrive, CreateJobRequest } from '../lib/types';
  import RsyncOptions from '../components/jobs/RsyncOptions.svelte';
  import SchedulePicker from '../components/jobs/SchedulePicker.svelte';
  import PathSelector from '../components/jobs/PathSelector.svelte';
  import SinglePathSelector from '../components/jobs/SinglePathSelector.svelte';
  import HelpTooltip from '../components/ui/HelpTooltip.svelte';
  import RunLogModal from '../components/logs/RunLogModal.svelte';
  import { confirm } from '../components/ui/ConfirmDialog.svelte';
  import { showToast } from '../components/ui/Toast.svelte';

  let { params = {} }: { params?: { id?: string } } = $props();

  let isNew = $derived(!params.id || params.id === 'new');
  let loading = $state(true);
  let saving = $state(false);
  let drives = $state<UsbDrive[]>([]);
  let schedules = $state<Schedule[]>([]);
  let loadedJobId = $state<string | null>(null);
  let activeRunId = $state<number | null>(null);
  let running = $state(false);
  let cloning = $state(false);

  // Form state
  let name = $state('');
  let description = $state('');
  let enabled = $state(true);
  let usbUuid = $state<string | null>(null);
  let mountPoint = $state('/mnt/backup');
  let autoMount = $state(true);
  let autoUnmount = $state(true);
  let sourceDirs = $state<string[]>([]);
  let backupSubdir = $state('backups');
  let syncDeletes = $state(false);
  let rsyncExcludes = $state<string[]>([]);
  let checksumMode = $state(false);
  let compress = $state(false);
  let dryRun = $state(false);
  let bandwidthLimit = $state<number | null>(null);
  let verbosity = $state<'quiet' | 'normal' | 'verbose'>('normal');

  async function loadData() {
    console.log('[JobDetail] loadData, params:', params, 'isNew:', isNew);
    loading = true;

    try {
      // Load USB drives if not already loaded
      if (drives.length === 0) {
        console.log('[JobDetail] Loading drives...');
        drives = await api.system.drives();
        console.log('[JobDetail] Drives loaded:', drives.length);
      }

      if (!isNew && params.id && params.id !== loadedJobId) {
        console.log('[JobDetail] Loading job:', params.id);
        // Set loadedJobId immediately to prevent re-entry from $effect
        loadedJobId = params.id;
        const job = await api.jobs.get(parseInt(params.id));
        console.log('[JobDetail] Job loaded:', job.name);
        loadJob(job);
        console.log('[JobDetail] Loading schedules...');
        schedules = await api.schedules.list(job.id);
        console.log('[JobDetail] Schedules loaded:', schedules.length);
      }
    } catch (e) {
      console.error('[JobDetail] Error:', e);
      showToast({ message: e instanceof Error ? e.message : 'Failed to load job', variant: 'error' });
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  // Reload when params change (handles navigation from /jobs/new to /jobs/:id)
  $effect(() => {
    const currentId = params.id;
    if (currentId && currentId !== 'new' && currentId !== loadedJobId) {
      console.log('[JobDetail] params changed, reloading:', currentId);
      loadData();
    }
  });

  function loadJob(job: Job) {
    name = job.name;
    description = job.description || '';
    enabled = job.enabled;
    usbUuid = job.usb_uuid;
    mountPoint = job.mount_point;
    autoMount = job.auto_mount;
    autoUnmount = job.auto_unmount;
    sourceDirs = job.source_dirs;
    backupSubdir = job.backup_subdir;
    syncDeletes = job.sync_deletes;
    rsyncExcludes = job.rsync_excludes;
    checksumMode = job.checksum_mode;
    compress = job.compress;
    dryRun = job.dry_run;
    bandwidthLimit = job.bandwidth_limit;
    verbosity = job.verbosity;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();

    // Validate at least one source directory is selected
    if (sourceDirs.length === 0) {
      showToast({ message: 'At least one source directory must be selected', variant: 'error' });
      return;
    }

    saving = true;

    const jobData: CreateJobRequest = {
      name,
      description: description || undefined,
      enabled,
      usb_uuid: usbUuid || undefined,
      mount_point: mountPoint,
      auto_mount: autoMount,
      auto_unmount: autoUnmount,
      source_dirs: sourceDirs,
      backup_subdir: backupSubdir,
      sync_deletes: syncDeletes,
      rsync_excludes: rsyncExcludes.length > 0 ? rsyncExcludes : undefined,
      checksum_mode: checksumMode,
      compress,
      dry_run: dryRun,
      bandwidth_limit: bandwidthLimit || undefined,
      verbosity,
    };

    console.log('[JobDetail] Submitting job:', jobData);

    try {
      if (isNew) {
        console.log('[JobDetail] Creating new job...');
        const job = await api.jobs.create(jobData);
        console.log('[JobDetail] Job created:', job);
        jobsStore.addJob(job);
        showToast({ message: 'Job created successfully', variant: 'success' });
      } else {
        console.log('[JobDetail] Updating job:', params.id);
        const job = await api.jobs.update(parseInt(params.id!), jobData);
        console.log('[JobDetail] Job updated:', job);
        jobsStore.updateJob(job);
        showToast({ message: 'Job saved successfully', variant: 'success' });
      }
      console.log('[JobDetail] Navigating to jobs list');
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

        <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
          <div class="relative flex items-center">
            <input type="checkbox" bind:checked={enabled} class="peer sr-only" />
            <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
            <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
          </div>
          <div class="flex-1 min-w-0">
            <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
              Job enabled
              <HelpTooltip text="When disabled, scheduled runs won't execute and the job won't appear in the run options. Useful for temporarily pausing backups without deleting the configuration." />
            </div>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">Enable or disable scheduled runs for this job.</p>
          </div>
        </label>
      </div>

      <!-- Mount Configuration -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Mount Configuration</h2>

        <div>
          <label for="usb" class="label">
            USB Drive (Optional)
            <HelpTooltip text="Select a USB drive to automatically mount before backup. The drive is identified by its unique UUID, so it will work regardless of which USB port you use. Leave as 'No USB mount' if backing up to a local folder or network drive." />
          </label>
          <select id="usb" bind:value={usbUuid} class="input">
            <option value={null}>No USB mount</option>
            {#each drives as drive}
              <option value={drive.uuid}>
                {drive.label || drive.name} ({drive.uuid.slice(0, 8)}...) - {drive.size}
              </option>
            {/each}
          </select>
        </div>

        <div>
          <label for="mount" class="label">
            Mount Point
            <HelpTooltip text="The directory path where your backup destination will be accessible. For USB drives, this is where the drive gets mounted (e.g., /mnt/backup). For local backups, this is simply the target folder path." />
          </label>
          <SinglePathSelector bind:path={mountPoint} placeholder="/mnt/backup" />
        </div>

        <div class="space-y-3">
          <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
            <div class="relative flex items-center">
              <input type="checkbox" bind:checked={autoMount} class="peer sr-only" />
              <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
              <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
            </div>
            <div class="flex-1 min-w-0">
              <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
                Auto-mount before backup
                <HelpTooltip text="Automatically mount the selected USB drive before the backup starts. The system will create the mount point directory if it doesn't exist. Only applies when a USB drive is selected above." />
              </div>
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">Mount the USB drive automatically when the job runs.</p>
            </div>
          </label>
          <label class="flex items-start gap-4 p-4 bg-gray-50 dark:bg-gray-900/50 rounded-xl cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/70 transition-colors">
            <div class="relative flex items-center">
              <input type="checkbox" bind:checked={autoUnmount} class="peer sr-only" />
              <div class="w-11 h-6 bg-gray-300 dark:bg-gray-600 rounded-full peer-checked:bg-primary-600 transition-colors"></div>
              <div class="absolute left-0.5 top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform peer-checked:translate-x-5"></div>
            </div>
            <div class="flex-1 min-w-0">
              <div class="font-medium text-gray-900 dark:text-white text-sm flex items-center gap-1">
                Auto-unmount after backup
                <HelpTooltip text="Safely unmount the USB drive after backup completes. This ensures all data is written to disk before the drive is disconnected, preventing data corruption." />
              </div>
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">Safely unmount the drive when the backup completes.</p>
            </div>
          </label>
        </div>
      </div>

      <!-- Source Directories -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
          Source Directories
          <HelpTooltip text="The folders on your system that you want to back up. You can add multiple directories, and each will be synced to a matching subfolder in the destination. Use the browser to navigate and select folders." />
        </h2>
        <PathSelector bind:paths={sourceDirs} />
      </div>

      <!-- Destination -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Destination</h2>
        <div>
          <label for="subdir" class="label">
            Backup Subdirectory
            <HelpTooltip text="A subfolder within the mount point where backups will be stored. This helps organize your backup drive, especially if you use it for multiple purposes. Each source directory will create its own folder inside this subdirectory." />
          </label>
          <input type="text" id="subdir" bind:value={backupSubdir} class="input" />
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
            Files will be backed up to: {mountPoint}/{backupSubdir}/
          </p>
        </div>
      </div>

      <!-- Rsync Options -->
      <div class="card p-6">
        <RsyncOptions
          bind:syncDeletes
          bind:checksumMode
          bind:compress
          bind:dryRun
          bind:bandwidthLimit
          bind:excludes={rsyncExcludes}
          bind:verbosity
        />
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
  <RunLogModal
    runId={activeRunId}
    jobId={parseInt(params.id!)}
    onClose={closeRunModal}
  />
{/if}

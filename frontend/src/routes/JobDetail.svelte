<script lang="ts">
  import { onMount } from 'svelte';
  import { push } from 'svelte-spa-router';
  import { api } from '../lib/api';
  import { jobsStore } from '../lib/stores/jobs';
  import type { Job, Schedule, UsbDrive, CreateJobRequest } from '../lib/types';
  import RsyncOptions from '../components/jobs/RsyncOptions.svelte';
  import SchedulePicker from '../components/jobs/SchedulePicker.svelte';
  import PathSelector from '../components/jobs/PathSelector.svelte';
  import SinglePathSelector from '../components/jobs/SinglePathSelector.svelte';

  let { params = {} }: { params?: { id?: string } } = $props();

  let isNew = $derived(!params.id || params.id === 'new');
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let drives = $state<UsbDrive[]>([]);
  let schedules = $state<Schedule[]>([]);
  let loadedJobId = $state<string | null>(null);

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

  async function loadData() {
    console.log('[JobDetail] loadData, params:', params, 'isNew:', isNew);
    loading = true;
    error = null;

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
      error = e instanceof Error ? e.message : 'Failed to load job';
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
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    saving = true;
    error = null;

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
    };

    console.log('[JobDetail] Submitting job:', jobData);

    try {
      if (isNew) {
        console.log('[JobDetail] Creating new job...');
        const job = await api.jobs.create(jobData);
        console.log('[JobDetail] Job created:', job);
        jobsStore.addJob(job);
        console.log('[JobDetail] Navigating to job:', job.id);
        push(`/jobs/${job.id}`);
      } else {
        console.log('[JobDetail] Updating job:', params.id);
        const job = await api.jobs.update(parseInt(params.id!), jobData);
        console.log('[JobDetail] Job updated:', job);
        jobsStore.updateJob(job);
      }
    } catch (e) {
      console.error('[JobDetail] Submit error:', e);
      error = e instanceof Error ? e.message : 'Failed to save job';
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!confirm('Are you sure you want to delete this job?')) return;

    try {
      await api.jobs.delete(parseInt(params.id!));
      jobsStore.removeJob(parseInt(params.id!));
      push('/jobs');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete job';
    }
  }

  async function handleRun() {
    try {
      const result = await api.jobs.run(parseInt(params.id!));
      alert(`Job started! Run ID: ${result.runId}`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to start job';
    }
  }
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-gray-900">
      {isNew ? 'New Backup Job' : 'Edit Backup Job'}
    </h1>
    {#if !isNew}
      <div class="flex gap-2">
        <button onclick={handleRun} class="btn btn-secondary">Run Now</button>
        <button onclick={handleDelete} class="btn btn-danger">Delete</button>
      </div>
    {/if}
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-primary-600"></div>
    </div>
  {:else}
    {#if error}
      <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg">
        {error}
      </div>
    {/if}

    <form onsubmit={handleSubmit} class="space-y-6">
      <!-- Basic Info -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900">Basic Information</h2>

        <div>
          <label for="name" class="label">Job Name</label>
          <input type="text" id="name" bind:value={name} required class="input" />
        </div>

        <div>
          <label for="description" class="label">Description</label>
          <textarea id="description" bind:value={description} rows="2" class="input"></textarea>
        </div>

        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={enabled} class="rounded text-primary-600" />
          <span class="text-sm text-gray-700">Job enabled</span>
        </label>
      </div>

      <!-- Mount Configuration -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900">Mount Configuration</h2>

        <div>
          <label for="usb" class="label">USB Drive (Optional)</label>
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
          <label for="mount" class="label">Mount Point</label>
          <SinglePathSelector bind:path={mountPoint} placeholder="/mnt/backup" />
        </div>

        <div class="flex gap-6">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={autoMount} class="rounded text-primary-600" />
            <span class="text-sm text-gray-700">Auto-mount before backup</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={autoUnmount} class="rounded text-primary-600" />
            <span class="text-sm text-gray-700">Auto-unmount after backup</span>
          </label>
        </div>
      </div>

      <!-- Source Directories -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900">Source Directories</h2>
        <PathSelector bind:paths={sourceDirs} />
      </div>

      <!-- Destination -->
      <div class="card p-6 space-y-4">
        <h2 class="text-lg font-semibold text-gray-900">Destination</h2>
        <div>
          <label for="subdir" class="label">Backup Subdirectory</label>
          <input type="text" id="subdir" bind:value={backupSubdir} class="input" />
          <p class="text-sm text-gray-500 mt-1">
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
        />
      </div>

      <!-- Schedule -->
      {#if !isNew}
        <div class="card p-6">
          <SchedulePicker jobId={parseInt(params.id!)} bind:schedules />
        </div>
      {/if}

      <!-- Actions -->
      <div class="flex justify-end gap-3">
        <a href="#/jobs" class="btn btn-secondary">Cancel</a>
        <button type="submit" disabled={saving} class="btn btn-primary">
          {saving ? 'Saving...' : isNew ? 'Create Job' : 'Save Changes'}
        </button>
      </div>
    </form>
  {/if}
</div>

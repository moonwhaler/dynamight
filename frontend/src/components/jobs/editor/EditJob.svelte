<script lang="ts">
  // Loads an existing job into a draft and shows the full editor for it.
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { Credential, Job, UsbDrive } from '$lib/types';
  import { JobDraft } from '$lib/jobDraft.svelte';
  import JobEditor from './JobEditor.svelte';
  import Spinner from '../../ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { jobId }: { jobId: number } = $props();

  const draft = new JobDraft();
  let drives = $state<UsbDrive[]>([]);
  let credentials = $state<Credential[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let mirrorWasOn = $state(false);

  async function load() {
    error = null;
    try {
      const [job, schedules] = await Promise.all([api.jobs.get(jobId), api.schedules.list(jobId)]);
      apply(job, schedules);
      loaded = true;
    } catch (e) {
      // Never show an editable form with another job's (or default) values
      error = e instanceof Error ? e.message : m.error_failed_load();
    }
  }

  function apply(job: Job, schedules: Awaited<ReturnType<typeof api.schedules.list>>) {
    draft.load(job, schedules);
    mirrorWasOn = !!job.sync_options?.delete_extraneous;
  }

  onMount(async () => {
    // Drives and credentials only fill pickers: a failure must not block editing
    const [d, c] = await Promise.allSettled([api.system.drives(), api.credentials.list()]);
    drives = d.status === 'fulfilled' ? d.value : [];
    credentials = c.status === 'fulfilled' ? c.value : [];
    await load();
  });
</script>

{#if error}
  <div class="card p-6 text-center space-y-4">
    <p class="text-red-600 dark:text-red-400">{error}</p>
    <button type="button" class="btn btn-secondary" onclick={load}>{m.common_retry()}</button>
  </div>
{:else if !loaded}
  <div class="flex justify-center py-16 text-primary-600"><Spinner size="w-6 h-6" /></div>
{:else}
  <JobEditor
    {draft}
    {jobId}
    {drives}
    bind:credentials
    {mirrorWasOn}
    onsaved={() => (mirrorWasOn = draft.syncOptions.delete_extraneous)}
    ondiscard={load}
  />
{/if}

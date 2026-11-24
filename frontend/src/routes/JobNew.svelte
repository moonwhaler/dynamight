<script lang="ts">
  import { onMount } from 'svelte';
  import { push, querystring } from 'svelte-spa-router';
  import { api } from '../lib/api';
  import type { Credential, UsbDrive } from '../lib/types';
  import { JobDraft, cloneName } from '../lib/jobDraft.svelte';
  import { preferencesStore } from '../lib/stores/preferences';
  import PageHeader from '../components/ui/PageHeader.svelte';
  import Spinner from '../components/ui/Spinner.svelte';
  import Icon from '../components/ui/Icon.svelte';
  import JobEditor from '../components/jobs/editor/JobEditor.svelte';
  import JobWizard from '../components/jobs/editor/JobWizard.svelte';
  import { showToast } from '../components/ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  const draft = new JobDraft();
  let drives = $state<UsbDrive[]>([]);
  let credentials = $state<Credential[]>([]);
  let ready = $state(false);
  const cloneId = $derived(Number(new URLSearchParams($querystring ?? '').get('clone')) || null);
  // A clone already has all its settings: show them all
  let mode = $state<'wizard' | 'full'>('full');

  onMount(async () => {
    const [d, c] = await Promise.allSettled([api.system.drives(), api.credentials.list()]);
    drives = d.status === 'fulfilled' ? d.value : [];
    credentials = c.status === 'fulfilled' ? c.value : [];
    if (cloneId) {
      try {
        const [job, schedules, jobs] = await Promise.all([api.jobs.get(cloneId), api.schedules.list(cloneId), api.jobs.list()]);
        draft.loadClone(job, schedules, cloneName(job.name, jobs.map((j) => j.name)));
      } catch (e) {
        showToast({ message: e instanceof Error ? e.message : m.error_failed_load(), variant: 'error' });
      }
    } else {
      mode = $preferencesStore.guidedJobSetup ? 'wizard' : 'full';
    }
    ready = true;
  });

  function saved(id: number) {
    push(`/jobs/${id}`);
  }
</script>

<a href="#/jobs" class="inline-flex items-center gap-1 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white mb-3">
  <Icon name="chevron-left" class="w-4 h-4" />{m.jobview_back()}
</a>
<PageHeader title={cloneId ? m.editor_clone_title() : m.editor_new_title()} />

{#if !ready}
  <div class="flex justify-center py-16 text-primary-600"><Spinner size="w-6 h-6" /></div>
{:else if mode === 'wizard'}
  <JobWizard {draft} {drives} bind:credentials onsaved={saved} onfullform={() => (mode = 'full')} />
{:else}
  <JobEditor {draft} jobId={null} {drives} bind:credentials onsaved={saved} />
{/if}

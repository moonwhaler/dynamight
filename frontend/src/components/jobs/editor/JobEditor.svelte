<script lang="ts">
  // The full job form: all sections on one page with a section list and a
  // save bar. Used to edit a job and, with guided setup off, to create one.
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { Credential, ProviderCapabilities, UsbDrive } from '$lib/types';
  import type { DraftSection, JobDraft } from '$lib/jobDraft.svelte';
  import { setLeaveGuard } from '$lib/leaveGuard';
  import { jobSummary } from '$lib/i18n/jobSummary';
  import EditorSection from './EditorSection.svelte';
  import DestinationFields from './DestinationFields.svelte';
  import { saveDraft } from './save';
  import PathSelector from '../PathSelector.svelte';
  import CompressOptions from '../CompressOptions.svelte';
  import SyncOptionsFields from '../SyncOptions.svelte';
  import SchedulePicker from '../SchedulePicker.svelte';
  import Toggle from '../../ui/Toggle.svelte';
  import Icon from '../../ui/Icon.svelte';
  import Spinner from '../../ui/Spinner.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    draft,
    jobId,
    drives,
    credentials = $bindable(),
    mirrorWasOn = false,
    onsaved,
    ondiscard,
  }: {
    draft: JobDraft;
    /** null while creating */
    jobId: number | null;
    drives: UsbDrive[];
    credentials: Credential[];
    mirrorWasOn?: boolean;
    onsaved?: (jobId: number) => void;
    ondiscard?: () => void;
  } = $props();

  const isNew = $derived(jobId === null);
  let saving = $state(false);
  let capabilities = $state<ProviderCapabilities | null>(null);
  let active = $state<string>('general');

  const SECTIONS: { id: string; label: () => string }[] = [
    { id: 'general', label: m.editor_section_general },
    { id: 'sources', label: m.editor_section_sources },
    { id: 'destination', label: m.editor_section_destination },
    { id: 'schedule', label: m.editor_section_schedule },
    { id: 'options', label: m.editor_section_options },
    { id: 'advanced', label: m.editor_section_advanced },
  ];

  $effect(() => {
    const type = draft.destinationType;
    api.providers
      .capabilities(type)
      .then((c) => (capabilities = c))
      .catch(() => (capabilities = null));
  });

  async function reloadCredentials() {
    try {
      credentials = await api.credentials.list();
    } catch {
      // Keep the list we have
    }
  }

  onMount(() => {
    const release = setLeaveGuard(() => draft.dirty && !saving);

    // Highlight the section in view
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
        if (visible[0]) active = visible[0].target.id;
      },
      { rootMargin: '-15% 0px -70% 0px' },
    );
    for (const s of SECTIONS) {
      const el = document.getElementById(s.id);
      if (el) observer.observe(el);
    }

    // Deep link to a section (e.g. "Edit" next to the schedules)
    const anchor = new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('section');
    if (anchor) setTimeout(() => goTo(anchor), 50);

    return () => {
      release();
      observer.disconnect();
    };
  });

  function goTo(id: string) {
    const el = document.getElementById(id);
    if (!el) return;
    if (el.tagName === 'DETAILS') (el as HTMLDetailsElement).open = true;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    active = id;
  }

  async function save(e?: Event) {
    e?.preventDefault();
    saving = true;
    const result = await saveDraft(draft, jobId, mirrorWasOn);
    saving = false;
    if (result.ok) onsaved?.(result.job.id);
    else if (result.invalid) goTo(result.invalid as DraftSection);
  }
</script>

<form onsubmit={save} class="lg:grid lg:grid-cols-[11rem_minmax(0,1fr)] lg:gap-10 items-start" novalidate>
  <!-- Section list: a sticky column on desktop, a scrolling chip row on smaller screens -->
  <nav aria-label={m.editor_sections()} class="lg:sticky lg:top-8 mb-5 lg:mb-0 -mx-4 px-4 sm:mx-0 sm:px-0 overflow-x-auto scrollbar-hide">
    <ul class="flex lg:flex-col gap-1.5 lg:gap-0.5 min-w-max lg:min-w-0">
      {#each SECTIONS as s (s.id)}
        <li>
          <button
            type="button"
            onclick={() => goTo(s.id)}
            aria-current={active === s.id ? 'true' : undefined}
            class="w-full text-left text-sm px-3 py-1.5 rounded-full lg:rounded-lg transition-colors whitespace-nowrap
              {active === s.id
              ? 'bg-gray-900 text-white dark:bg-white dark:text-gray-900 lg:bg-gray-200/80 lg:text-gray-900 lg:dark:bg-gray-800 lg:dark:text-white font-medium'
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/60 dark:hover:bg-gray-800 border border-gray-300 dark:border-gray-700 lg:border-0'}"
          >
            {s.label()}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="min-w-0 space-y-6 max-w-3xl">
    <p class="text-sm text-gray-600 dark:text-gray-300 bg-gray-100 dark:bg-gray-800/60 rounded-lg px-4 py-3">
      {jobSummary(draft.toRequest(), draft.activeCrons)}
    </p>

    <EditorSection id="general" title={m.editor_section_general()}>
      <div class="space-y-4">
        <div>
          <label for="job-name" class="label">{m.job_label_name()}</label>
          <input id="job-name" class="input" bind:value={draft.name} required autocomplete="off" />
        </div>
        <div>
          <label for="job-desc" class="label">{m.job_label_description()} <span class="font-normal text-gray-400">({m.common_optional()})</span></label>
          <textarea id="job-desc" class="input" rows="2" bind:value={draft.description}></textarea>
        </div>
        <Toggle
          label={m.job_status_enabled()}
          description={draft.enabled ? m.job_status_enabled_desc() : m.job_status_disabled_desc()}
          bind:checked={draft.enabled}
        />
      </div>
    </EditorSection>

    <EditorSection id="sources" title={m.editor_section_sources()} description={m.job_sources_help()}>
      <div class="space-y-6">
        <PathSelector bind:paths={draft.sourceDirs} />
        <div class="pt-6 border-t border-gray-200 dark:border-gray-700">
          <SyncOptionsFields bind:options={draft.syncOptions} destinationType={draft.destinationType} {capabilities} sourceDirs={draft.sourceDirs} part="excludes" />
        </div>
        <div class="pt-6 border-t border-gray-200 dark:border-gray-700">
          <CompressOptions bind:options={draft.syncOptions} sourceDirs={draft.sourceDirs} destinationType={draft.destinationType} />
        </div>
      </div>
    </EditorSection>

    <EditorSection id="destination" title={m.editor_section_destination()}>
      <DestinationFields {draft} {drives} {credentials} onCredentialsChange={reloadCredentials} compact={!isNew} />
    </EditorSection>

    <EditorSection id="schedule" title={m.editor_section_schedule()} description={m.schedule_help()}>
      <SchedulePicker bind:schedules={draft.schedules} bind:pendingSchedules={draft.pendingSchedules} dirty={!isNew && draft.schedulesDirty} />
    </EditorSection>

    <EditorSection id="options" title={m.editor_section_options()}>
      <SyncOptionsFields bind:options={draft.syncOptions} destinationType={draft.destinationType} {capabilities} sourceDirs={draft.sourceDirs} part="basic" />
    </EditorSection>

    <details id="advanced" class="card group scroll-mt-24">
      <summary class="flex items-center justify-between gap-3 p-5 sm:p-6 cursor-pointer list-none [&::-webkit-details-marker]:hidden">
        <span>
          <span class="block text-base font-semibold text-gray-900 dark:text-white">{m.editor_section_advanced()}</span>
          <span class="block text-sm text-gray-500 dark:text-gray-400 mt-1">{m.editor_advanced_desc()}</span>
        </span>
        <Icon name="chevron-down" class="w-5 h-5 text-gray-400 transition-transform group-open:rotate-180" />
      </summary>
      <div class="px-5 sm:px-6 pb-6">
        <SyncOptionsFields bind:options={draft.syncOptions} destinationType={draft.destinationType} {capabilities} sourceDirs={draft.sourceDirs} part="advanced" />
      </div>
    </details>

    <!-- Save bar: stays at the bottom of the screen while the form scrolls -->
    <div class="sticky bottom-[calc(4.5rem+env(safe-area-inset-bottom))] md:bottom-4 z-20">
      <div
        class="flex items-center gap-3 px-4 py-3 rounded-xl border shadow-lg backdrop-blur
          {draft.dirty || isNew
          ? 'bg-white/95 dark:bg-gray-800/95 border-gray-300 dark:border-gray-600'
          : 'bg-white/80 dark:bg-gray-800/80 border-gray-200 dark:border-gray-700'}"
      >
        <p class="flex-1 text-sm {draft.dirty ? 'text-gray-900 dark:text-white font-medium' : 'text-gray-500 dark:text-gray-400'}">
          {#if isNew}{m.editor_not_created()}{:else if draft.dirty}{m.editor_unsaved()}{:else}{m.editor_saved()}{/if}
        </p>
        {#if !isNew && draft.dirty}
          <button type="button" class="btn btn-ghost" onclick={() => ondiscard?.()} disabled={saving}>{m.editor_discard()}</button>
        {/if}
        <button type="submit" class="btn btn-primary" disabled={saving || (!isNew && !draft.dirty)}>
          {#if saving}<Spinner />{/if}
          {isNew ? m.job_btn_add() : m.job_btn_save()}
        </button>
      </div>
    </div>
  </div>
</form>

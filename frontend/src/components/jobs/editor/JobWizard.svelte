<script lang="ts">
  // Guided setup for a new job: one question per step, then a review.
  import { onMount, type Snippet } from 'svelte';
  import { api } from '$lib/api';
  import type { Credential, ProviderCapabilities, UsbDrive } from '$lib/types';
  import type { JobDraft } from '$lib/jobDraft.svelte';
  import { setLeaveGuard } from '$lib/leaveGuard';
  import { jobSummary } from '$lib/i18n/jobSummary';
  import { formatCron } from '$lib/i18n/schedule';
  import { destinationKind, destinationTarget } from '$lib/utils/jobUtils';
  import { destinationKindLabel } from '$lib/utils/destination';
  import DestinationFields from './DestinationFields.svelte';
  import { saveDraft } from './save';
  import PathSelector from '../PathSelector.svelte';
  import SyncOptionsFields from '../SyncOptions.svelte';
  import SchedulePicker from '../SchedulePicker.svelte';
  import PathText from '../../ui/PathText.svelte';
  import Icon from '../../ui/Icon.svelte';
  import Spinner from '../../ui/Spinner.svelte';
  import { showToast } from '../../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    draft,
    drives,
    credentials = $bindable(),
    onsaved,
    onfullform,
  }: {
    draft: JobDraft;
    drives: UsbDrive[];
    credentials: Credential[];
    onsaved?: (jobId: number) => void;
    /** Switch to the full form, keeping what was entered */
    onfullform?: () => void;
  } = $props();

  type Step = 'basics' | 'sources' | 'destination' | 'schedule' | 'review';
  const STEPS: { id: Step; label: () => string; validates?: 'general' | 'sources' | 'destination' }[] = [
    { id: 'basics', label: m.wizard_step_basics, validates: 'general' },
    { id: 'sources', label: m.wizard_step_sources, validates: 'sources' },
    { id: 'destination', label: m.wizard_step_destination, validates: 'destination' },
    { id: 'schedule', label: m.wizard_step_schedule },
    { id: 'review', label: m.wizard_step_review },
  ];

  let index = $state(0);
  let saving = $state(false);
  let capabilities = $state<ProviderCapabilities | null>(null);
  let heading = $state<HTMLElement | null>(null);
  const step = $derived(STEPS[index]);

  $effect(() => {
    api.providers
      .capabilities(draft.destinationType)
      .then((c) => (capabilities = c))
      .catch(() => (capabilities = null));
  });

  onMount(() => setLeaveGuard(() => draft.dirty && !saving));

  function go(to: number) {
    index = to;
    // Move focus to the new step's title so keyboard and screen reader users follow
    queueMicrotask(() => heading?.focus());
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  function next() {
    const problem = step.validates ? draft.validate(step.validates) : null;
    if (problem) {
      showToast({ message: problem, variant: 'error' });
      return;
    }
    go(index + 1);
  }

  async function create() {
    saving = true;
    const result = await saveDraft(draft, null, false);
    saving = false;
    if (result.ok) onsaved?.(result.job.id);
    else if (result.invalid) go(STEPS.findIndex((s) => s.validates === result.invalid));
  }

  async function reloadCredentials() {
    try {
      credentials = await api.credentials.list();
    } catch {
      // Keep the list we have
    }
  }

  const kind = $derived(destinationKind({ destination: draft.destination, destination_type: draft.destinationType }));
</script>

<div class="max-w-2xl">
  <!-- Progress: a real sequence, so the steps are numbered -->
  <ol class="grid grid-cols-5 gap-1.5 mb-2" aria-label={m.wizard_progress()}>
    {#each STEPS as s, i (s.id)}
      <li>
        <button
          type="button"
          class="block w-full h-1.5 rounded-full transition-colors {i <= index ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-700'} disabled:cursor-default"
          disabled={i >= index}
          onclick={() => go(i)}
          aria-label={m.wizard_step_n({ n: i + 1, label: s.label() })}
          aria-current={i === index ? 'step' : undefined}
        ></button>
      </li>
    {/each}
  </ol>
  <p class="eyebrow mb-6">{m.wizard_step_of({ n: index + 1, total: STEPS.length })} · {step.label()}</p>

  <h2 bind:this={heading} tabindex="-1" class="font-display text-xl sm:text-2xl text-gray-900 dark:text-white mb-1 outline-none">
    {#if step.id === 'basics'}{m.wizard_q_basics()}
    {:else if step.id === 'sources'}{m.wizard_q_sources()}
    {:else if step.id === 'destination'}{m.wizard_q_destination()}
    {:else if step.id === 'schedule'}{m.wizard_q_schedule()}
    {:else}{m.wizard_q_review()}{/if}
  </h2>
  <p class="text-sm text-gray-500 dark:text-gray-400 mb-6">
    {#if step.id === 'basics'}{m.wizard_h_basics()}
    {:else if step.id === 'sources'}{m.wizard_h_sources()}
    {:else if step.id === 'destination'}{m.wizard_h_destination()}
    {:else if step.id === 'schedule'}{m.wizard_h_schedule()}
    {:else}{m.wizard_h_review()}{/if}
  </p>

  <div class="card p-5 sm:p-6">
    {#if step.id === 'basics'}
      <div class="space-y-4">
        <div>
          <label for="w-name" class="label">{m.job_label_name()}</label>
          <input
            id="w-name"
            class="input"
            bind:value={draft.name}
            placeholder={m.wizard_name_placeholder()}
            autocomplete="off"
            onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), next())}
          />
        </div>
        <div>
          <label for="w-desc" class="label">{m.job_label_description()} <span class="font-normal text-gray-400">({m.common_optional()})</span></label>
          <textarea id="w-desc" class="input" rows="2" bind:value={draft.description}></textarea>
        </div>
      </div>
    {:else if step.id === 'sources'}
      <div class="space-y-6">
        <PathSelector bind:paths={draft.sourceDirs} />
        <details class="group pt-5 border-t border-gray-200 dark:border-gray-700">
          <summary class="flex items-center gap-2 cursor-pointer text-sm font-medium text-gray-700 dark:text-gray-300 list-none [&::-webkit-details-marker]:hidden">
            <Icon name="chevron-right" class="w-4 h-4 transition-transform group-open:rotate-90" />{m.wizard_exclude_toggle()}
          </summary>
          <div class="mt-5">
            <SyncOptionsFields bind:options={draft.syncOptions} destinationType={draft.destinationType} {capabilities} sourceDirs={draft.sourceDirs} part="excludes" />
          </div>
        </details>
      </div>
    {:else if step.id === 'destination'}
      <DestinationFields {draft} {drives} {credentials} onCredentialsChange={reloadCredentials} />
    {:else if step.id === 'schedule'}
      <SchedulePicker bind:schedules={draft.schedules} bind:pendingSchedules={draft.pendingSchedules} />
    {:else}
      <div class="space-y-6">
        <p class="text-[15px] leading-relaxed text-gray-800 dark:text-gray-200">{jobSummary(draft.toRequest(), draft.activeCrons)}</p>
        <dl class="rows text-sm border-y border-gray-200 dark:border-gray-700">
          {#snippet row(label: string, to: number, content: Snippet)}
            <div class="flex items-start gap-4 py-3">
              <dt class="w-28 shrink-0 text-gray-500 dark:text-gray-400">{label}</dt>
              <dd class="flex-1 min-w-0 text-gray-900 dark:text-white">{@render content()}</dd>
              <button type="button" class="text-primary-700 dark:text-primary-400 hover:underline shrink-0" onclick={() => go(to)}>{m.common_edit()}</button>
            </div>
          {/snippet}
          {#snippet nameC()}{draft.name}{/snippet}
          {#snippet sourcesC()}
            {#each draft.sourceDirs as d (d)}<PathText path={d} max={40} class="block truncate" />{/each}
          {/snippet}
          {#snippet destC()}
            {destinationKindLabel(kind)} · <PathText path={destinationTarget({ destination: draft.destination })} max={32} />
          {/snippet}
          {#snippet schedC()}
            {draft.activeCrons.length ? draft.activeCrons.map(formatCron).join(', ') : m.overview_manual_only()}
          {/snippet}
          {@render row(m.job_label_name(), 0, nameC)}
          {@render row(m.jobview_sources(), 1, sourcesC)}
          {@render row(m.jobview_destination(), 2, destC)}
          {@render row(m.jobview_schedules(), 3, schedC)}
        </dl>
        <SyncOptionsFields bind:options={draft.syncOptions} destinationType={draft.destinationType} {capabilities} sourceDirs={draft.sourceDirs} part="basic" />
      </div>
    {/if}
  </div>

  <div class="flex flex-col-reverse sm:flex-row sm:items-center gap-3 mt-6">
    <button type="button" class="btn btn-ghost sm:mr-auto text-sm" onclick={() => onfullform?.()}>{m.wizard_full_form()}</button>
    {#if index > 0}
      <button type="button" class="btn btn-secondary" onclick={() => go(index - 1)}>
        <Icon name="chevron-left" class="w-4 h-4" />{m.common_back()}
      </button>
    {/if}
    {#if step.id === 'review'}
      <button type="button" class="btn btn-primary" onclick={create} disabled={saving}>
        {#if saving}<Spinner />{/if}{m.job_btn_add()}
      </button>
    {:else}
      <button type="button" class="btn btn-primary" onclick={next}>
        {step.id === 'schedule' && draft.activeCrons.length === 0 ? m.wizard_skip() : m.common_next()}
        <Icon name="chevron-right" class="w-4 h-4" />
      </button>
    {/if}
  </div>
</div>

<script lang="ts">
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    path,
    root,
    editing = $bindable(false),
    compact = false,
    onNavigate,
  }: {
    /** The open folder */
    path: string;
    /** The place it belongs to; nothing above it is shown */
    root: string | null;
    /** Shows a text field for typing a path */
    editing?: boolean;
    /** Narrow screens: only the open folder, the rest in the "…" menu */
    compact?: boolean;
    onNavigate: (path: string) => void;
  } = $props();

  interface Crumb {
    label: string;
    path: string;
  }

  /** Segments from the place down to the open folder */
  const crumbs = $derived.by<Crumb[]>(() => {
    const base = root ?? '/';
    const rest = path.slice(base.length).split('/').filter(Boolean);
    const list: Crumb[] = [{ label: base.split('/').filter(Boolean).pop() ?? '/', path: base }];
    let acc = base.replace(/\/+$/, '');
    for (const seg of rest) {
      acc = `${acc}/${seg}`;
      list.push({ label: seg, path: acc });
    }
    return list;
  });

  // With many levels: the place, "…", and the last two
  const MAX_SHOWN = 4;
  const hidden = $derived(
    compact ? crumbs.slice(0, -1) : crumbs.length > MAX_SHOWN ? crumbs.slice(1, crumbs.length - 2) : [],
  );
  const shown = $derived(
    compact ? crumbs.slice(-1) : crumbs.length > MAX_SHOWN ? [crumbs[0], ...crumbs.slice(-2)] : crumbs,
  );
  /** Index in `shown` before which the "…" menu sits */
  const menuAt = $derived(compact ? 0 : 1);
  const hiddenItems = $derived<MenuItem[]>(
    hidden.map((c) => ({ label: c.label, icon: 'folder', onclick: () => onNavigate(c.path) })),
  );

  let draft = $state('');
  let input = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (editing) {
      draft = path;
      tick().then(() => input?.select());
    }
  });

  function submit() {
    const target = draft.trim().replace(/(.)\/+$/, '$1');
    editing = false;
    if (target && target !== path) onNavigate(target);
  }
</script>

{#if editing}
  <form
    class="flex-1 min-w-0"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <label for="files-path" class="sr-only">{m.filebrowser_path_label()}</label>
    <input
      bind:this={input}
      id="files-path"
      class="input !py-1.5 font-mono text-sm"
      bind:value={draft}
      autocomplete="off"
      spellcheck="false"
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          editing = false;
        }
      }}
      onblur={() => (editing = false)}
    />
  </form>
{:else}
  <!-- Clicking the empty space after the last segment edits the path -->
  <div
    class="flex-1 min-w-0 flex items-center rounded-lg hover:bg-gray-100/70 dark:hover:bg-gray-800/60 cursor-text"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) editing = true;
    }}
  >
    <nav aria-label={m.filebrowser_path_label()} class="min-w-0 flex items-center">
      <ol class="flex items-center min-w-0 text-sm">
        {#each shown as crumb, i (crumb.path)}
          {@const last = i === shown.length - 1}
          {#if i > 0}
            <li aria-hidden="true" class="text-gray-300 dark:text-gray-600 px-0.5 shrink-0">/</li>
          {/if}
          {#if i === menuAt && hidden.length > 0}
            <li class="shrink-0">
              <Menu label={m.filebrowser_path_more()} items={hiddenItems} align="start" triggerClass="btn btn-ghost btn-sm px-1.5">
                {#snippet trigger()}…{/snippet}
              </Menu>
            </li>
            <li aria-hidden="true" class="text-gray-300 dark:text-gray-600 px-0.5 shrink-0">/</li>
          {/if}
          <li class="min-w-0 {last ? 'shrink' : 'shrink-[2]'}">
            {#if last}
              <span class="block px-1.5 py-1 font-medium text-gray-900 dark:text-white truncate" aria-current="location" title={crumb.path}>
                {crumb.label}
              </span>
            {:else}
              <button
                type="button"
                class="block max-w-full px-1.5 py-1 rounded-md text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/70 dark:hover:bg-gray-700/60 truncate"
                title={crumb.path}
                onclick={() => onNavigate(crumb.path)}
              >
                {crumb.label}
              </button>
            {/if}
          </li>
        {/each}
      </ol>
    </nav>
    <button
      type="button"
      class="ml-auto btn btn-ghost btn-icon btn-sm shrink-0 opacity-60 hover:opacity-100"
      onclick={() => (editing = true)}
      aria-label={m.filebrowser_path_edit()}
      title={m.filebrowser_path_edit()}
    >
      <Icon name="edit" class="w-4 h-4" />
    </button>
  </div>
{/if}

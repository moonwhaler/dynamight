<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { DirectoryEntry } from '$lib/types';
  import type { SortField, SortOrder } from '$lib/files/fileSort';
  import { virtualWindow } from '$lib/files/virtualWindow';
  import { rangeOf, selectionState } from '$lib/files/selection';
  import { api } from '$lib/api';
  import type { SvelteSet } from 'svelte/reactivity';
  import { formatModified, formatModifiedFull } from '$lib/files/fileDates';
  import { getFileIcon } from '$lib/utils/fileIcons';
  import { formatBytes } from '$lib/utils/format';
  import { getLocale } from '$lib/paraglide/runtime.js';
  import Icon from '../ui/Icon.svelte';
  import ContextMenu from '../ui/ContextMenu.svelte';
  import SortIcon from '../ui/SortIcon.svelte';
  import type { MenuItem } from '../ui/Menu.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    entries,
    view,
    sortBy,
    sortOrder,
    loading,
    error,
    sizesLoading = false,
    searchQuery = '',
    searchBase = '',
    emptyText,
    onSort,
    onRetry,
    onOpen,
    menuFor,
    onDelete,
    onUp,
    onRename,
    selected,
    thumbs = false,
    onActiveChange,
  }: {
    entries: DirectoryEntry[];
    view: 'list' | 'grid';
    sortBy: SortField;
    sortOrder: SortOrder;
    loading: boolean;
    error: string | null;
    sizesLoading?: boolean;
    /** Highlighted in names */
    searchQuery?: string;
    /** Subfolder search: each row also shows its folder below this path */
    searchBase?: string;
    emptyText: string;
    onSort: (field: SortField) => void;
    onRetry: () => void;
    /** Folder: open it; file: its default action */
    onOpen: (entry: DirectoryEntry) => void;
    menuFor: (entry: DirectoryEntry) => MenuItem[];
    onDelete: (entry: DirectoryEntry) => void;
    onUp: () => void;
    /** F2 */
    onRename?: (entry: DirectoryEntry) => void;
    /** Selected paths; without it the list has no selection */
    selected?: SvelteSet<string>;
    /** Image and video thumbnails in the grid */
    thumbs?: boolean;
    /** The row moved to with the keyboard (the preview follows it) */
    onActiveChange?: (entry: DirectoryEntry) => void;
  } = $props();

  const uid = `fl-${Math.random().toString(36).slice(2, 8)}`;
  const locale = getLocale();
  const now = new Date();

  let container = $state<HTMLDivElement | null>(null);
  let width = $state(0);
  let offset = $state(0);
  let viewport = $state(800);
  let coarse = $state(false);
  let active = $state(-1);

  // The list's own width counts (the preview panel narrows it on desktops)
  const phone = $derived(width > 0 && width < 540);
  const selecting = $derived((selected?.size ?? 0) > 0);
  const allState = $derived(selected ? selectionState(entries.map((e) => e.path), selected) : 'none');
  /** Where a Shift-click range starts */
  let anchor = -1;
  /** Thumbnails that failed to load (the icon shows instead) */
  let noThumb = $state(new Set<string>());
  const rowHeight = $derived(phone ? 56 : (coarse ? 44 : 36) + (searchBase ? 18 : 0));
  const TILE_WIDTH = 150;
  const TILE_HEIGHT = 150;
  const GAP = 12;
  const columns = $derived(view === 'grid' ? Math.max(2, Math.floor((width - 16 + GAP) / (TILE_WIDTH + GAP))) : 1);
  const lineHeight = $derived(view === 'grid' ? TILE_HEIGHT + GAP : rowHeight);
  const lineCount = $derived(Math.ceil(entries.length / columns));
  const win = $derived(virtualWindow(offset, viewport, lineHeight, lineCount, view === 'grid' ? 4 : 20));
  const visible = $derived(
    entries.slice(win.start * columns, win.end * columns).map((entry, i) => ({ entry, index: win.start * columns + i })),
  );

  // A new listing starts without an active row
  $effect(() => {
    void entries;
    if (active >= entries.length) active = entries.length - 1;
  });

  function measure() {
    if (!container) return;
    offset = Math.max(0, -container.getBoundingClientRect().top);
    viewport = window.innerHeight;
  }

  onMount(() => {
    const media = window.matchMedia('(pointer: coarse)');
    coarse = media.matches;
    const onMedia = () => (coarse = media.matches);
    media.addEventListener('change', onMedia);

    let frame = 0;
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(() => ((frame = 0), measure()));
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    const ro = new ResizeObserver(() => {
      width = container?.clientWidth ?? 0;
      schedule();
    });
    if (container) ro.observe(container);
    measure();
    return () => {
      media.removeEventListener('change', onMedia);
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
      ro.disconnect();
      cancelAnimationFrame(frame);
    };
  });

  // Content above the list (search bar) moves it
  $effect(() => {
    void entries.length;
    void searchBase;
    tick().then(measure);
  });

  // ── Keyboard ──

  /** Scroll the window so row `index` is visible */
  function reveal(index: number) {
    if (!container) return;
    const top = container.getBoundingClientRect().top + window.scrollY + (view === 'list' && !phone ? 40 : 0);
    const rowTop = top + Math.floor(index / columns) * lineHeight;
    const stickyTop = window.innerWidth < 768 ? 104 : 48;
    if (rowTop < window.scrollY + stickyTop) window.scrollTo({ top: rowTop - stickyTop });
    else if (rowTop + lineHeight > window.scrollY + window.innerHeight - 96)
      window.scrollTo({ top: rowTop + lineHeight - window.innerHeight + 96 });
  }

  function setActive(index: number, extend = false) {
    if (entries.length === 0) return;
    const next = Math.max(0, Math.min(entries.length - 1, index));
    if (extend && selected) {
      if (anchor < 0) anchor = active < 0 ? next : active;
      selectRange(next);
    }
    active = next;
    reveal(active);
    onActiveChange?.(entries[active]);
  }

  // ── Selection ──

  function toggle(index: number) {
    if (!selected || !entries[index]) return;
    const path = entries[index].path;
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
    anchor = index;
  }

  function selectRange(index: number) {
    if (!selected) return;
    for (const path of rangeOf(entries.map((e) => e.path), anchor < 0 ? index : anchor, index)) selected.add(path);
  }

  function toggleAll() {
    if (!selected) return;
    if (allState === 'all') selected.clear();
    else for (const e of entries) selected.add(e.path);
  }

  function onKeydown(e: KeyboardEvent) {
    const step = view === 'grid' ? columns : 1;
    const current = active < 0 ? -1 : active;
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setActive(current < 0 ? 0 : current + step, e.shiftKey);
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (e.altKey) onUp();
        else setActive(current < 0 ? 0 : current - step, e.shiftKey);
        break;
      case ' ':
        if (selected && entries[current]) {
          e.preventDefault();
          toggle(current);
        }
        break;
      case 'a':
        if ((e.ctrlKey || e.metaKey) && selected) {
          e.preventDefault();
          for (const entry of entries) selected.add(entry.path);
        }
        break;
      case 'Escape':
        if (selecting) {
          e.preventDefault();
          e.stopPropagation();
          selected?.clear();
        }
        break;
      case 'ArrowRight':
        if (view === 'grid') {
          e.preventDefault();
          setActive(current + 1);
        }
        break;
      case 'ArrowLeft':
        if (view === 'grid') {
          e.preventDefault();
          setActive(current - 1);
        }
        break;
      case 'PageDown':
        e.preventDefault();
        setActive(current + step * 10);
        break;
      case 'PageUp':
        e.preventDefault();
        setActive(current - step * 10);
        break;
      case 'Home':
        e.preventDefault();
        setActive(0);
        break;
      case 'End':
        e.preventDefault();
        setActive(entries.length - 1);
        break;
      case 'Enter':
        if (entries[current]) {
          e.preventDefault();
          onOpen(entries[current]);
        }
        break;
      case 'Backspace':
        e.preventDefault();
        onUp();
        break;
      case 'F2':
        if (onRename && entries[current]) {
          e.preventDefault();
          onRename(entries[current]);
        }
        break;
      case 'Delete':
        if (entries[current]) {
          e.preventDefault();
          onDelete(entries[current]);
        }
        break;
      case 'ContextMenu':
      case 'F10':
        if ((e.key === 'ContextMenu' || e.shiftKey) && entries[current]) {
          e.preventDefault();
          const row = document.getElementById(`${uid}-${current}`);
          const r = row?.getBoundingClientRect();
          openMenu(entries[current], r ? r.left + 48 : 100, r ? r.bottom : 100);
        }
        break;
    }
  }

  // ── Menu ──

  let menuOpen = $state(false);
  let menuAt = $state({ x: 0, y: 0 });
  let menuItems = $state<MenuItem[]>([]);
  let menuLabel = $state('');

  function openMenu(entry: DirectoryEntry, x: number, y: number) {
    menuItems = menuFor(entry);
    menuLabel = m.filebrowser_actions_for({ name: entry.name });
    menuAt = { x, y };
    menuOpen = true;
  }

  // Long press on touch screens selects (or, without selection, opens the menu)
  let pressTimer: ReturnType<typeof setTimeout> | null = null;
  let pressed = false;
  function pressStart(entry: DirectoryEntry, index: number, e: TouchEvent) {
    pressed = false;
    const t = e.touches[0];
    pressTimer = setTimeout(() => {
      pressed = true;
      active = index;
      if (selected) toggle(index);
      else openMenu(entry, t.clientX, t.clientY);
      navigator.vibrate?.(15);
    }, 500);
  }
  function pressEnd() {
    if (pressTimer) clearTimeout(pressTimer);
    pressTimer = null;
  }

  function onRowClick(entry: DirectoryEntry, index: number, e: MouseEvent) {
    if (pressed) {
      pressed = false;
      return;
    }
    container?.focus({ preventScroll: true });
    if (selected && e.shiftKey) {
      selectRange(index);
      active = index;
      return;
    }
    if (selected && (e.ctrlKey || e.metaKey || (coarse && selecting))) {
      // Ctrl-click, or a tap while selecting on a touch screen
      toggle(index);
      active = index;
      return;
    }
    active = index;
    anchor = index;
    onOpen(entry);
  }

  // ── Display helpers ──

  function segments(text: string): { text: string; hit: boolean }[] {
    const q = searchQuery.trim().toLowerCase();
    const at = q ? text.toLowerCase().indexOf(q) : -1;
    if (at < 0) return [{ text, hit: false }];
    return [
      { text: text.slice(0, at), hit: false },
      { text: text.slice(at, at + q.length), hit: true },
      { text: text.slice(at + q.length), hit: false },
    ].filter((s) => s.text);
  }

  function folderOf(entry: DirectoryEntry): string {
    if (!searchBase) return '';
    const dir = entry.path.split('/').slice(0, -1).join('/');
    return dir === searchBase ? '' : dir.startsWith(`${searchBase}/`) ? dir.slice(searchBase.length + 1) : dir;
  }

  const sortable: { field: SortField; label: () => string; align: string }[] = [
    { field: 'name', label: m.filebrowser_column_name, align: '' },
    { field: 'size', label: m.filebrowser_column_size, align: 'justify-end' },
    { field: 'modified', label: m.filebrowser_column_modified, align: 'justify-end' },
  ];
  const checkColumn = $derived(!!selected && (!phone || selecting));
  const listColumns = $derived(
    phone
      ? `grid-template-columns: ${checkColumn ? '2rem ' : ''}minmax(0,1fr) 2.75rem`
      : `grid-template-columns: ${checkColumn ? '2rem ' : ''}minmax(0,1fr) 6.5rem 8.5rem 2.75rem`,
  );

  function hasThumb(entry: DirectoryEntry): boolean {
    if (!thumbs || entry.is_dir || noThumb.has(entry.path)) return false;
    const kind = getFileIcon(entry.extension, false).icon;
    return kind === 'image' || kind === 'video';
  }
</script>

{#snippet size(entry: DirectoryEntry)}
  {#if entry.is_dir && entry.size == null && sizesLoading}
    <span class="inline-block h-2.5 w-12 rounded bg-gray-200 dark:bg-gray-700 animate-pulse align-middle" aria-label={m.filebrowser_size_loading()}></span>
  {:else if entry.size == null}
    <span class="text-gray-400 dark:text-gray-500">—</span>
  {:else}
    {formatBytes(entry.size)}
  {/if}
{/snippet}

{#snippet name(entry: DirectoryEntry)}
  {#each segments(entry.name) as seg, i (i)}
    {#if seg.hit}<mark class="bg-primary-100 dark:bg-primary-900/50 text-inherit rounded-sm">{seg.text}</mark>{:else}{seg.text}{/if}
  {/each}
{/snippet}

{#snippet moreButton(entry: DirectoryEntry, index: number)}
  <button
    type="button"
    tabindex="-1"
    class="btn btn-ghost btn-icon btn-sm {coarse || index === active ? '' : 'opacity-0 group-hover:opacity-100 focus:opacity-100'}"
    aria-label={m.filebrowser_actions_for({ name: entry.name })}
    title={m.filebrowser_actions()}
    onclick={(e) => {
      e.stopPropagation();
      active = index;
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      openMenu(entry, r.right - 8, r.bottom);
    }}
  >
    <Icon name="more" class="w-4 h-4" />
  </button>
{/snippet}

{#snippet checkbox(entry: DirectoryEntry, index: number)}
  {@const checked = selected?.has(entry.path) ?? false}
  <input
    type="checkbox"
    tabindex="-1"
    class="w-4 h-4 rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500 cursor-pointer
      {checked || selecting || coarse ? '' : 'opacity-0 group-hover:opacity-100'}"
    {checked}
    aria-label={m.filebrowser_select_item({ name: entry.name })}
    onclick={(e) => {
      e.stopPropagation();
      if ((e as MouseEvent).shiftKey) {
        selectRange(index);
        (e.currentTarget as HTMLInputElement).checked = true;
      } else toggle(index);
      active = index;
    }}
  />
{/snippet}

{#if loading && entries.length === 0}
  <div class="py-16 flex justify-center" role="status" aria-label={m.filebrowser_loading()}>
    <Icon name="refresh" class="w-6 h-6 text-primary-500 animate-spin" />
  </div>
{:else if error}
  <div class="py-14 px-6 text-center" role="alert">
    <Icon name="warning" class="w-8 h-8 mx-auto text-red-500 mb-3" />
    <p class="text-sm text-gray-700 dark:text-gray-300">{error}</p>
    <button type="button" class="btn btn-secondary btn-sm mt-4" onclick={onRetry}>
      <Icon name="refresh" class="w-4 h-4" />{m.common_retry()}
    </button>
  </div>
{:else if entries.length === 0}
  <div class="py-16 px-6 text-center text-sm text-gray-500 dark:text-gray-400">
    <Icon name={searchQuery.trim() ? 'search' : 'folder-open'} class="w-8 h-8 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
    {emptyText}
  </div>
{/if}

<div
  bind:this={container}
  class="outline-none focus-visible:ring-2 focus-visible:ring-primary-500/50 rounded-b-xl {entries.length === 0 || error ? 'hidden' : ''}"
  role="grid"
  tabindex="0"
  aria-label={m.filebrowser_list_label()}
  aria-rowcount={entries.length + 1}
  aria-colcount={view === 'list' ? 4 : 1}
  aria-activedescendant={active >= 0 && active < entries.length ? `${uid}-${active}` : undefined}
  aria-busy={loading}
  onkeydown={onKeydown}
  onfocus={() => {
    if (active < 0 && entries.length) active = 0;
  }}
>
  {#if view === 'list'}
    {#if !phone}
      <div
        role="row"
        aria-rowindex={1}
        class="sticky top-12 md:top-0 z-10 grid items-center h-10 px-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50/95 dark:bg-gray-800/95 backdrop-blur text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400"
        style={listColumns}
      >
        {#if checkColumn}
          <div role="columnheader" class="flex">
            <input
              type="checkbox"
              tabindex="-1"
              class="w-4 h-4 rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500 cursor-pointer"
              checked={allState === 'all'}
              indeterminate={allState === 'some'}
              aria-label={m.filebrowser_select_all()}
              onclick={toggleAll}
            />
          </div>
        {/if}
        {#each sortable as col (col.field)}
          <div
            role="columnheader"
            class="flex {col.align}"
            aria-sort={sortBy === col.field ? (sortOrder === 'asc' ? 'ascending' : 'descending') : 'none'}
          >
            <button
              type="button"
              tabindex="-1"
              class="flex items-center gap-1 px-1 -mx-1 rounded hover:text-gray-900 dark:hover:text-white"
              onclick={() => onSort(col.field)}
            >
              {col.label()}
              <SortIcon active={sortBy === col.field} order={sortOrder} />
            </button>
          </div>
        {/each}
        <div role="columnheader"><span class="sr-only">{m.filebrowser_actions()}</span></div>
      </div>
    {/if}
    <div style="padding-top: {win.padTop}px; padding-bottom: {win.padBottom}px">
      {#each visible as { entry, index } (entry.path)}
        {@const icon = getFileIcon(entry.extension, entry.is_dir)}
        {@const folder = folderOf(entry)}
        <!-- The grid handles the keys and points at the active row (aria-activedescendant) -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          id="{uid}-{index}"
          role="row"
          tabindex="-1"
          aria-rowindex={index + 2}
          aria-selected={selected ? selected.has(entry.path) : index === active}
          class="group grid items-center px-3 border-b border-gray-100 dark:border-gray-800 last:border-b-0 cursor-pointer select-none
            {selected?.has(entry.path)
            ? 'bg-primary-100/70 dark:bg-primary-900/35'
            : index === active
              ? 'bg-primary-50/70 dark:bg-primary-900/20'
              : 'hover:bg-gray-50 dark:hover:bg-gray-800/60'}
            {index === active ? 'shadow-[inset_2px_0_0_0] shadow-primary-500' : ''}"
          style="height: {rowHeight}px; {listColumns}"
          onclick={(e) => onRowClick(entry, index, e)}
          oncontextmenu={(e) => {
            e.preventDefault();
            active = index;
            openMenu(entry, e.clientX, e.clientY);
          }}
          ontouchstart={(e) => pressStart(entry, index, e)}
          ontouchend={pressEnd}
          ontouchmove={pressEnd}
        >
          {#if checkColumn}
            <div role="gridcell" class="flex">{@render checkbox(entry, index)}</div>
          {/if}
          <div role="gridcell" class="flex items-center gap-3 min-w-0">
            <Icon name={icon.icon} class="w-5 h-5 shrink-0 {icon.color}" />
            <div class="min-w-0">
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="truncate text-sm {entry.is_dir ? 'font-medium text-gray-900 dark:text-white' : 'text-gray-800 dark:text-gray-200'}" title={entry.name}>
                  {@render name(entry)}
                </span>
                {#if entry.is_symlink}
                  <Icon name="link" class="w-3.5 h-3.5 shrink-0 text-gray-400" />
                  <span class="sr-only">{m.filebrowser_symlink()}</span>
                {/if}
              </div>
              {#if folder}
                <div class="truncate text-xs text-gray-500 dark:text-gray-400" title={folder}>{folder}</div>
              {:else if phone}
                <div class="truncate text-xs text-gray-500 dark:text-gray-400 tabular-nums">
                  {[!entry.is_dir || entry.size != null ? formatBytes(entry.size) : '', formatModified(entry.modified, now, locale)]
                    .filter(Boolean)
                    .join(' · ')}
                </div>
              {/if}
            </div>
          </div>
          {#if !phone}
            <div role="gridcell" class="text-right text-sm tabular-nums text-gray-600 dark:text-gray-400">{@render size(entry)}</div>
            <div role="gridcell" class="text-right text-sm tabular-nums text-gray-600 dark:text-gray-400 truncate" title={formatModifiedFull(entry.modified, locale)}>
              {formatModified(entry.modified, now, locale)}
            </div>
          {/if}
          <div role="gridcell" class="flex justify-end">{@render moreButton(entry, index)}</div>
        </div>
      {/each}
    </div>
  {:else}
    <div
      class="grid gap-3 p-2"
      style="grid-template-columns: repeat({columns}, minmax(0, 1fr)); padding-top: {win.padTop + 8}px; padding-bottom: {win.padBottom + 8}px"
    >
      {#each visible as { entry, index } (entry.path)}
        {@const icon = getFileIcon(entry.extension, entry.is_dir)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          id="{uid}-{index}"
          role="row"
          tabindex="-1"
          aria-rowindex={index + 2}
          aria-selected={selected ? selected.has(entry.path) : index === active}
          class="group relative flex flex-col items-center justify-center gap-2 px-2 rounded-xl border cursor-pointer select-none
            {selected?.has(entry.path)
            ? 'border-primary-500 bg-primary-100/70 dark:bg-primary-900/35'
            : index === active
              ? 'border-primary-400 bg-primary-50/70 dark:bg-primary-900/20'
              : 'border-transparent hover:bg-gray-50 dark:hover:bg-gray-800/60'}"
          style="height: {TILE_HEIGHT}px"
          onclick={(e) => onRowClick(entry, index, e)}
          oncontextmenu={(e) => {
            e.preventDefault();
            active = index;
            openMenu(entry, e.clientX, e.clientY);
          }}
          ontouchstart={(e) => pressStart(entry, index, e)}
          ontouchend={pressEnd}
          ontouchmove={pressEnd}
        >
          <div role="gridcell" class="contents">
            {#if hasThumb(entry)}
              <img
                src={api.files.thumbUrl(entry.path)}
                alt=""
                loading="lazy"
                decoding="async"
                class="w-full h-[76px] object-cover rounded-lg bg-gray-100 dark:bg-gray-800"
                onerror={() => (noThumb = new Set(noThumb).add(entry.path))}
              />
            {:else}
              <div class="w-full h-[76px] flex items-center justify-center">
                <Icon name={icon.icon} class="w-12 h-12 {icon.color}" strokeWidth={1.25} />
              </div>
            {/if}
            <span class="w-full text-center text-sm leading-tight line-clamp-2 break-words text-gray-800 dark:text-gray-200" title={entry.name}>
              {@render name(entry)}
            </span>
            <span class="text-xs tabular-nums text-gray-500 dark:text-gray-400">
              {#if entry.is_dir && entry.size == null}{formatModified(entry.modified, now, locale)}{:else}{@render size(entry)}{/if}
            </span>
            {#if selected}<div class="absolute top-2 left-2">{@render checkbox(entry, index)}</div>{/if}
            <div class="absolute top-1 right-1">{@render moreButton(entry, index)}</div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<ContextMenu bind:open={menuOpen} x={menuAt.x} y={menuAt.y} label={menuLabel} items={menuItems} returnFocus={container} />

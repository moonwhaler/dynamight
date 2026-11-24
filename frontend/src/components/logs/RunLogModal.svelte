<script lang="ts">
  // The log of one run: live while it runs, paged once it is done, with
  // server-side search and level filter, copy and download.
  import { onMount, onDestroy, tick } from 'svelte';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import type { LogEntry, JobRun } from '../../lib/types';
  import { stopJob } from '../../lib/jobActions';
  import { runDurationMs, runHasResult } from '../../lib/utils/jobStatus';
  import { formatStatus, getStatusBadgeClass } from '$lib/i18n/status';
  import { formatDateTime, formatDuration } from '$lib/i18n/relativeTime';
  import { formatBytes } from '$lib/utils/format';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from '../ui/Icon.svelte';
  import Spinner from '../ui/Spinner.svelte';
  import { showToast } from '../ui/Toast.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let { runId, jobId, onClose }: { runId: number; jobId: number; onClose: () => void } = $props();

  const PAGE = 500;
  const LIVE_MAX = 2000;
  type LevelFilter = 'all' | 'warnings' | 'errors';

  let open = $state(true);
  let run = $state<JobRun | null>(null);
  let lines = $state<LogEntry[]>([]);
  let total = $state(0);
  let offset = $state(0);
  let loading = $state(false);
  let query = $state('');
  let appliedQuery = $state('');
  let level = $state<LevelFilter>('all');
  let follow = $state(true);
  let downloading = $state(false);
  let stopping = $state(false);
  let ws: WebSocket | null = null;
  let poll: ReturnType<typeof setInterval> | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let scroller = $state<HTMLDivElement | null>(null);

  const jobName = $derived($jobsStore.jobs.find((j) => j.id === jobId)?.name ?? `#${jobId}`);
  const running = $derived(run?.status === 'running' || run?.status === 'pending');
  const filtered = $derived(level !== 'all' || appliedQuery !== '');
  /** New lines stream in over the socket only while running and unfiltered */
  const live = $derived(running && !filtered);
  const levels = $derived(level === 'errors' ? ['error'] : level === 'warnings' ? ['warning', 'error'] : []);
  const duration = $derived(run ? runDurationMs(run) : null);

  onMount(async () => {
    follow = $preferencesStore.autoShowLastPage;
    await loadRun();
    if (running) {
      connect();
      await loadTail();
    } else {
      await loadLastPage();
    }
    poll = setInterval(async () => {
      const was = running;
      await loadRun();
      if (was && !running) {
        disconnect();
        await loadLastPage();
      } else if (running && filtered) {
        await loadPage(offset, false);
      }
    }, 2000);
  });

  onDestroy(() => {
    disconnect();
    if (poll) clearInterval(poll);
    if (searchTimer) clearTimeout(searchTimer);
  });

  async function loadRun() {
    try {
      run = await api.runs.get(runId);
    } catch {
      // Keep what we have
    }
  }

  function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${window.location.host}/api/ws/logs/${runId}`);
    ws.onmessage = (event) => {
      if (filtered) return;
      try {
        const msg = JSON.parse(event.data);
        const entry: LogEntry = {
          id: -Date.now() - Math.random(),
          job_run_id: msg.run_id,
          timestamp: msg.timestamp,
          level: msg.level,
          message: msg.message,
          source: msg.source,
        };
        lines = lines.length >= LIVE_MAX ? [...lines.slice(1), entry] : [...lines, entry];
        total = Math.max(total + 1, lines.length);
        if (follow) scrollToEnd();
      } catch {
        // Not a log line
      }
    };
    ws.onclose = () => (ws = null);
    ws.onerror = () => ws?.close();
  }

  function disconnect() {
    ws?.close();
    ws = null;
  }

  /** While running: what was logged so far, then live lines after it */
  async function loadTail() {
    try {
      const first = await api.runs.logs(runId, 1, 0);
      const start = Math.max(0, first.total - PAGE);
      const page = await api.runs.logs(runId, PAGE, start);
      // Lines that came over the socket meanwhile may be in the page too
      const key = (e: LogEntry) => `${e.level}|${e.message}`;
      const seen = new Set(page.entries.slice(-200).map(key));
      lines = [...page.entries, ...lines.filter((e) => !seen.has(key(e)))].slice(-LIVE_MAX);
      total = Math.max(page.total, lines.length);
      offset = start;
      scrollToEnd();
    } catch {
      // Live lines still arrive
    }
  }

  async function loadPage(at: number, scroll = true) {
    loading = true;
    try {
      const page = await api.runs.logs(runId, PAGE, at, { levels, q: appliedQuery });
      lines = page.entries;
      total = page.total;
      offset = at;
      if (scroll && scroller) scroller.scrollTop = 0;
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      loading = false;
    }
  }

  async function loadLastPage() {
    try {
      const { total: t } = await api.runs.logs(runId, 1, 0, { levels, q: appliedQuery });
      await loadPage(Math.max(0, Math.floor(Math.max(0, t - 1) / PAGE) * PAGE), false);
      if (follow) scrollToEnd();
    } catch {
      await loadPage(0);
    }
  }

  /** Filters changed: show matches from the start; unfiltered and running: back to live */
  async function applyFilters() {
    if (!filtered && running) {
      lines = [];
      await loadTail();
    } else if (filtered) {
      await loadPage(0);
    } else {
      await loadLastPage();
    }
  }

  function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      appliedQuery = query.trim();
      applyFilters();
    }, 300);
  }

  function setLevel(l: LevelFilter) {
    level = l;
    applyFilters();
  }

  async function scrollToEnd() {
    await tick();
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  function onScroll() {
    if (!scroller || !live) return;
    const atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
    if (!atEnd && follow) follow = false;
  }

  function lineText(e: LogEntry): string {
    return `${e.timestamp} ${e.level.toUpperCase().padEnd(7)} ${e.source ? `[${e.source}] ` : ''}${e.message}`;
  }

  async function copyShown() {
    try {
      await navigator.clipboard.writeText(lines.map(lineText).join('\n'));
      showToast({ message: m.runlog_copied({ count: lines.length }), variant: 'success' });
    } catch {
      showToast({ message: m.error_generic(), variant: 'error' });
    }
  }

  async function download() {
    downloading = true;
    try {
      const all: LogEntry[] = [];
      for (let at = 0; ; at += 5000) {
        const page = await api.runs.logs(runId, 5000, at, { levels, q: appliedQuery });
        all.push(...page.entries);
        if (!page.has_more || page.entries.length === 0) break;
      }
      const blob = new Blob([all.map(lineText).join('\n') + '\n'], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `dynamight-${jobName.replace(/[^\w.-]+/g, '_')}-run-${runId}.log`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      showToast({ message: e instanceof Error ? e.message : m.error_generic(), variant: 'error' });
    } finally {
      downloading = false;
    }
  }

  async function stop() {
    const job = $jobsStore.jobs.find((j) => j.id === jobId);
    if (!job) return;
    stopping = true;
    if (await stopJob(job)) await loadRun();
    stopping = false;
  }

  /** Split a message around the search text for highlighting */
  function parts(message: string): { text: string; hit: boolean }[] {
    if (!appliedQuery) return [{ text: message, hit: false }];
    const out: { text: string; hit: boolean }[] = [];
    const lower = message.toLowerCase();
    const needle = appliedQuery.toLowerCase();
    let i = 0;
    for (let at = lower.indexOf(needle); at !== -1; at = lower.indexOf(needle, i)) {
      if (at > i) out.push({ text: message.slice(i, at), hit: false });
      out.push({ text: message.slice(at, at + needle.length), hit: true });
      i = at + needle.length;
    }
    if (i < message.length) out.push({ text: message.slice(i), hit: false });
    return out;
  }

  const levelTone: Record<LogEntry['level'], string> = {
    debug: 'text-gray-500',
    info: 'text-sky-300',
    warning: 'text-amber-300',
    error: 'text-red-400',
  };

  function time(ts: string): string {
    const d = new Date(ts);
    return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
</script>

<Dialog
  bind:open
  title={`${jobName} · ${m.run_number({ id: runId })}`}
  description={run?.started_at ? formatDateTime(run.started_at) : undefined}
  size="full"
  bodyClass="p-0 flex flex-col min-h-0"
  onclose={onClose}
>
  {#snippet header()}
    {#if run}
      <span class="badge {getStatusBadgeClass(run.status)} self-center hidden sm:inline-flex">{formatStatus(run.status)}</span>
    {/if}
  {/snippet}

  <!-- Facts and stop -->
  <div class="flex flex-wrap items-center gap-x-5 gap-y-1 px-5 sm:px-6 py-2.5 text-xs text-gray-600 dark:text-gray-300 border-b border-gray-200 dark:border-gray-700 tabular-nums">
    {#if run}
      <span class="sm:hidden badge {getStatusBadgeClass(run.status)}">{formatStatus(run.status)}</span>
      {#if duration != null}<span>{running ? m.runlog_running_for({ value: formatDuration(duration) }) : m.runlog_duration({ value: formatDuration(duration) })}</span>{/if}
      {#if runHasResult(run)}
        <span>{m.run_files({ count: run.files_transferred ?? 0 })}</span>
        <span>{formatBytes(run.bytes_transferred ?? 0)}</span>
      {/if}
      {#if run.error_count > 0}<span class="text-red-600 dark:text-red-400">{m.runlog_errors({ count: run.error_count })}</span>{/if}
      {#if run.summary && run.status !== 'completed'}
        <span class="font-mono truncate max-w-full {run.status === 'failed' ? 'text-red-600 dark:text-red-400' : ''}" title={run.summary}>{run.summary}</span>
      {/if}
      {#if running}
        <button type="button" class="btn btn-danger btn-sm ml-auto" onclick={stop} disabled={stopping}>
          {#if stopping}<Spinner />{m.job_btn_stopping()}{:else}<Icon name="stop" class="w-4 h-4" />{m.runlog_stop()}{/if}
        </button>
      {/if}
    {/if}
  </div>

  <!-- Toolbar -->
  <div class="flex flex-wrap items-center gap-2 px-5 sm:px-6 py-2.5 border-b border-gray-200 dark:border-gray-700">
    <div class="relative flex-1 min-w-[12rem]">
      <Icon name="search" class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none" />
      <input
        type="search"
        class="input h-9 pl-9 text-sm"
        placeholder={m.runlog_search()}
        aria-label={m.runlog_search()}
        bind:value={query}
        oninput={onSearchInput}
      />
    </div>
    <div class="inline-flex p-0.5 rounded-lg bg-gray-100 dark:bg-gray-900/60" role="radiogroup" aria-label={m.runlog_level()}>
      {#each [{ id: 'all', label: m.common_all() }, { id: 'warnings', label: m.runlog_level_warnings() }, { id: 'errors', label: m.runlog_level_errors() }] as const as opt (opt.id)}
        <button
          type="button"
          role="radio"
          aria-checked={level === opt.id}
          onclick={() => setLevel(opt.id)}
          class="h-8 px-3 rounded-md text-sm transition-colors {level === opt.id
            ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm font-medium'
            : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}">{opt.label}</button
        >
      {/each}
    </div>
    <div class="flex items-center gap-1 ml-auto">
      {#if live}
        <button
          type="button"
          class="btn btn-sm {follow ? 'btn-secondary' : 'btn-ghost'}"
          aria-pressed={follow}
          onclick={() => {
            follow = !follow;
            if (follow) scrollToEnd();
          }}
        >
          <Icon name="arrows" class="w-4 h-4 rotate-90" />{m.runlog_follow()}
        </button>
      {/if}
      <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={copyShown} aria-label={m.runlog_copy()} title={m.runlog_copy()}>
        <Icon name="copy" class="w-4 h-4" />
      </button>
      <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={download} disabled={downloading} aria-label={m.runlog_download()} title={m.runlog_download()}>
        {#if downloading}<Spinner />{:else}<Icon name="download" class="w-4 h-4" />{/if}
      </button>
    </div>
  </div>

  <!-- Lines -->
  <div
    bind:this={scroller}
    onscroll={onScroll}
    class="flex-1 min-h-0 overflow-auto bg-gray-950 text-gray-200 font-mono text-[12.5px] leading-relaxed py-2"
    role="log"
    aria-live={live ? 'polite' : 'off'}
  >
    {#if loading && lines.length === 0}
      <div class="flex justify-center py-12 text-gray-400"><Spinner size="w-6 h-6" /></div>
    {:else if lines.length === 0}
      <p class="px-5 sm:px-6 py-8 text-gray-400 font-sans text-sm">{filtered ? m.runlog_no_match() : m.runlog_empty()}</p>
    {:else}
      {#each lines as e (e.id)}
        <div class="flex gap-3 px-5 sm:px-6 hover:bg-white/5 {e.level === 'error' ? 'bg-red-500/10' : ''}">
          <span class="shrink-0 text-gray-500 select-none">{time(e.timestamp)}</span>
          <span class="shrink-0 w-12 uppercase text-[11px] pt-px {levelTone[e.level]}">{e.level === 'warning' ? 'warn' : e.level}</span>
          <span class="min-w-0 whitespace-pre-wrap break-all">
            {#if e.source}<span class="text-violet-300">[{e.source}]</span>{/if}
            {#each parts(e.message) as p, i (i)}{#if p.hit}<mark class="bg-amber-300 text-gray-950 rounded-sm px-px">{p.text}</mark>{:else}{p.text}{/if}{/each}
          </span>
        </div>
      {/each}
    {/if}
  </div>

  <!-- Paging (finished or filtered logs) -->
  {#if !live && total > PAGE}
    <div class="flex items-center justify-between gap-3 px-5 sm:px-6 py-2 border-t border-gray-200 dark:border-gray-700 text-sm tabular-nums">
      <button type="button" class="btn btn-ghost btn-sm" disabled={offset === 0 || loading} onclick={() => loadPage(Math.max(0, offset - PAGE))}>
        <Icon name="chevron-left" class="w-4 h-4" />{m.runlog_earlier()}
      </button>
      <span class="text-gray-500 dark:text-gray-400">{m.runlog_range({ from: offset + 1, to: Math.min(offset + PAGE, total), total })}</span>
      <button type="button" class="btn btn-ghost btn-sm" disabled={offset + PAGE >= total || loading} onclick={() => loadPage(offset + PAGE)}>
        {m.runlog_later()}<Icon name="chevron-right" class="w-4 h-4" />
      </button>
    </div>
  {/if}
</Dialog>

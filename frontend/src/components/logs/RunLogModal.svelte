<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import { preferencesStore } from '../../lib/stores/preferences';
  import { get } from 'svelte/store';
  import type { LogEntry, JobRun } from '../../lib/types';
  import LogViewer from './LogViewer.svelte';
  import { confirm } from '../ui/ConfirmDialog.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    runId,
    jobId,
    onClose,
  }: {
    runId: number;
    jobId: number;
    onClose: () => void;
  } = $props();

  // Portal action - moves element to document.body to avoid DOM hierarchy issues
  // (e.g., when rendered inside a table, the fixed positioning and events break)
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      }
    };
  }

  let logs = $state<LogEntry[]>([]);
  let logsTotal = $state(0);
  let logsCurrentPage = $state(1);
  let loadingLogs = $state(false);
  let run = $state<JobRun | null>(null);
  let ws: WebSocket | null = null;
  let killing = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  const LOG_PAGE_SIZE = 500;

  const logsTotalPages = $derived(Math.max(1, Math.ceil(logsTotal / LOG_PAGE_SIZE)));

  const jobName = $derived($jobsStore.jobs.find((j) => j.id === jobId)?.name || `Job #${jobId}`);

  const isRunning = $derived(run?.status === 'running' || run?.status === 'pending');

  onMount(() => {
    connectWebSocket();
    loadRunStatus();
    // Poll for run status updates
    pollInterval = setInterval(loadRunStatus, 2000);
  });

  onDestroy(() => {
    disconnectWebSocket();
    if (pollInterval) {
      clearInterval(pollInterval);
    }
  });

  async function connectWebSocket() {
    // Get token for WebSocket authentication
    let token: string;
    try {
      const response = await api.auth.getToken();
      token = response.token;
    } catch {
      console.error('Failed to get token for WebSocket');
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    ws = new WebSocket(`${protocol}//${host}/api/ws/logs/${runId}?token=${encodeURIComponent(token)}`);

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        const entry: LogEntry = {
          id: Date.now() + Math.random(),
          job_run_id: msg.run_id,
          timestamp: msg.timestamp,
          level: msg.level,
          message: msg.message,
          source: msg.source,
        };
        // Limit logs to last 2000 entries, removing oldest from top
        if (logs.length >= 2000) {
          logs = [...logs.slice(1), entry];
        } else {
          logs = [...logs, entry];
        }
      } catch {
        // Ignore parse errors
      }
    };

    ws.onclose = () => {
      ws = null;
    };

    ws.onerror = () => {
      ws?.close();
      ws = null;
    };
  }

  function disconnectWebSocket() {
    if (ws) {
      ws.close();
      ws = null;
    }
  }

  async function loadRunStatus() {
    try {
      run = await api.runs.get(runId);
      // Stop polling if job is no longer running
      if (run && !['running', 'pending'].includes(run.status)) {
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
        killing = false;
        // Load final logs from API (first page)
        await loadLogsPage(1);
      }
    } catch {
      // Ignore
    }
  }

  async function loadLogsPage(page: number) {
    loadingLogs = true;
    try {
      const offset = (page - 1) * LOG_PAGE_SIZE;
      const response = await api.runs.logs(runId, LOG_PAGE_SIZE, offset);
      logs = response.entries;
      logsTotal = response.total;
      logsCurrentPage = page;
    } catch {
      // Ignore
    } finally {
      loadingLogs = false;
    }
  }

  function handlePageChange(page: number) {
    loadLogsPage(page);
  }

  async function handleKill() {
    if (killing) return;

    // Check if confirmation is required
    if (get(preferencesStore).confirmKillProcess) {
      const confirmed = await confirm({
        title: m.kill_confirm_title(),
        message: m.kill_confirm_message(),
        confirmText: m.kill_confirm_button(),
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    killing = true;
    try {
      await api.jobs.cancel(jobId);
      await loadRunStatus();
    } catch {
      // Ignore
    }
  }

  function getStatusBadge(status: string): string {
    switch (status) {
      case 'completed':
        return 'badge-success';
      case 'running':
        return 'badge-info';
      case 'failed':
        return 'badge-error';
      case 'cancelled':
        return 'badge-warning';
      default:
        return 'badge-gray';
    }
  }
</script>

<div
  use:portal
  class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-2 sm:p-4 overflow-hidden"
  onwheel={(e) => e.stopPropagation()}
>
  <div class="bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-[95vw] lg:max-w-[85vw] xl:max-w-7xl h-[95vh] sm:h-[90vh] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="p-3 sm:p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between gap-2 shrink-0">
      <div class="flex items-center gap-2 sm:gap-3 min-w-0 flex-1">
        <div class="min-w-0 flex-1">
          <h3 class="text-base sm:text-lg font-semibold text-gray-900 dark:text-white truncate">
            {jobName}
          </h3>
          <div class="flex items-center gap-2 text-xs sm:text-sm text-gray-500 dark:text-gray-400">
            <span>Run #{runId}</span>
            {#if run}
              <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
            {/if}
          </div>
        </div>
        {#if isRunning}
          <div class="flex-shrink-0">
            <div class="animate-spin rounded-full h-4 w-4 sm:h-5 sm:w-5 border-b-2 border-primary-600"></div>
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-2 flex-shrink-0">
        {#if isRunning || killing}
          <button
            onclick={() => handleKill()}
            disabled={killing}
            class="btn btn-danger text-xs sm:text-sm py-1.5 px-2 sm:px-3"
          >
            {killing ? 'Killing...' : 'Kill'}
          </button>
        {/if}
        <button
          onclick={onClose}
          class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
          aria-label="Close"
        >
          <svg class="w-5 h-5 sm:w-6 sm:h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>
    </div>

    <!-- Stats bar (shown when job has stats) -->
    {#if run && run.files_transferred !== null}
      <div class="px-3 sm:px-4 py-2 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-700 flex flex-wrap gap-3 sm:gap-6 text-xs sm:text-sm shrink-0">
        <div>
          <span class="text-gray-500 dark:text-gray-400">Files:</span>
          <span class="font-medium text-gray-900 dark:text-white">{run.files_transferred}</span>
        </div>
        {#if run.bytes_transferred !== null}
          <div>
            <span class="text-gray-500 dark:text-gray-400">Size:</span>
            <span class="font-medium text-gray-900 dark:text-white">
              {(run.bytes_transferred / (1024 * 1024)).toFixed(1)} MB
            </span>
          </div>
        {/if}
        {#if run.error_count > 0}
          <div>
            <span class="text-gray-500 dark:text-gray-400">Errors:</span>
            <span class="font-medium text-red-600 dark:text-red-400">{run.error_count}</span>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Log viewer -->
    <div class="flex-1 min-h-0 overflow-hidden relative">
      <div class="absolute inset-0">
        <LogViewer
          {logs}
          total={logsTotal}
          currentPage={logsCurrentPage}
          totalPages={logsTotalPages}
          loading={loadingLogs}
          pageSize={LOG_PAGE_SIZE}
          onPageChange={handlePageChange}
          isStreaming={isRunning}
        />
      </div>
    </div>

    <!-- Footer -->
    <div class="p-3 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex justify-end shrink-0">
      <button onclick={onClose} class="btn btn-secondary text-sm">
        Close
      </button>
    </div>
  </div>
</div>

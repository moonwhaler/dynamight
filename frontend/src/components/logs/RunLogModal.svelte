<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../../lib/api';
  import { jobsStore } from '../../lib/stores/jobs';
  import type { LogEntry, JobRun } from '../../lib/types';
  import LogViewer from './LogViewer.svelte';

  let {
    runId,
    jobId,
    onClose,
  }: {
    runId: number;
    jobId: number;
    onClose: () => void;
  } = $props();

  let logs = $state<LogEntry[]>([]);
  let run = $state<JobRun | null>(null);
  let ws: WebSocket | null = null;
  let cancelling = $state(false);
  let cancelRequestedAt = $state<number | null>(null);
  let forceKilling = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let currentTime = $state(Date.now());

  const jobName = $derived($jobsStore.jobs.find((j) => j.id === jobId)?.name || `Job #${jobId}`);

  const isRunning = $derived(run?.status === 'running' || run?.status === 'pending');

  // Show force kill option after 3 seconds of being in "Stopping..." state
  const showForceKill = $derived(
    cancelling && cancelRequestedAt && currentTime - cancelRequestedAt > 3000
  );

  // Reactive duration that updates with currentTime
  const runningDuration = $derived.by(() => {
    if (!run?.started_at) return '-';
    const startDate = new Date(run.started_at);
    const endDate = run.completed_at ? new Date(run.completed_at) : new Date(currentTime);
    const diff = Math.floor((endDate.getTime() - startDate.getTime()) / 1000);
    if (diff < 60) return `${diff}s`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ${diff % 60}s`;
    return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
  });

  let timeInterval: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    connectWebSocket();
    loadRunStatus();
    // Poll for run status updates
    pollInterval = setInterval(loadRunStatus, 2000);
    // Update current time for force kill timer
    timeInterval = setInterval(() => {
      currentTime = Date.now();
    }, 500);
  });

  onDestroy(() => {
    disconnectWebSocket();
    if (pollInterval) {
      clearInterval(pollInterval);
    }
    if (timeInterval) {
      clearInterval(timeInterval);
    }
  });

  function connectWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    ws = new WebSocket(`${protocol}//${host}/api/ws/logs/${runId}`);

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
        logs = [...logs, entry];
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
        // Load final logs from API to ensure we have everything
        const finalLogs = await api.runs.logs(runId);
        if (finalLogs.length > logs.length) {
          logs = finalLogs;
        }
      }
    } catch {
      // Ignore
    }
  }

  async function handleCancel() {
    if (cancelling) return;
    cancelling = true;
    cancelRequestedAt = Date.now();
    try {
      const result = await api.jobs.cancel(jobId, false);
      await loadRunStatus();
      // If process was killed or status changed, reset cancelling state
      if (result.processKilled || !isRunning) {
        cancelling = false;
        cancelRequestedAt = null;
      }
    } catch {
      // Ignore
    }
  }

  async function handleForceKill() {
    if (forceKilling) return;
    forceKilling = true;
    try {
      await api.jobs.cancel(jobId, true);
      await loadRunStatus();
      // Reset all cancellation state after force kill
      cancelling = false;
      cancelRequestedAt = null;
    } catch {
      // Ignore errors
    } finally {
      forceKilling = false;
    }
  }

  function formatDate(date: string | null): string {
    if (!date) return '';
    return new Date(date).toLocaleString();
  }

  function formatDuration(start: string | null, end: string | null): string {
    if (!start) return '-';
    const startDate = new Date(start);
    const endDate = end ? new Date(end) : new Date();
    const diff = Math.floor((endDate.getTime() - startDate.getTime()) / 1000);
    if (diff < 60) return `${diff}s`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ${diff % 60}s`;
    return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
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
  class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4 overflow-hidden"
  onwheel={(e) => e.stopPropagation()}
>
  <div class="bg-white rounded-xl shadow-xl max-w-4xl w-full h-[90vh] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="p-4 border-b flex items-center justify-between shrink-0">
      <div class="flex items-center gap-3">
        <div>
          <h3 class="text-lg font-semibold text-gray-900">
            {jobName}
          </h3>
          <div class="flex items-center gap-2 text-sm text-gray-500">
            <span>Run #{runId}</span>
            {#if run}
              <span class="badge {getStatusBadge(run.status)}">{run.status}</span>
              {#if run.started_at}
                <span>• {runningDuration}</span>
              {/if}
            {/if}
          </div>
        </div>
        {#if isRunning}
          <div class="ml-2">
            <div class="animate-spin rounded-full h-5 w-5 border-b-2 border-primary-600"></div>
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        {#if isRunning || cancelling}
          {#if showForceKill}
            <button
              onclick={() => handleForceKill()}
              disabled={forceKilling}
              class="btn btn-danger text-sm py-1.5 px-3 animate-pulse"
              title="Force kill the process immediately"
            >
              {forceKilling ? 'Killing...' : 'Force Kill'}
            </button>
          {:else}
            <button
              onclick={() => handleCancel()}
              disabled={cancelling}
              class="btn btn-danger text-sm py-1.5 px-3"
            >
              {cancelling ? 'Stopping...' : 'Stop'}
            </button>
          {/if}
        {/if}
        <button
          onclick={onClose}
          class="text-gray-400 hover:text-gray-600 p-1"
          aria-label="Close"
        >
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
      <div class="px-4 py-2 bg-gray-50 border-b flex gap-6 text-sm shrink-0">
        <div>
          <span class="text-gray-500">Files:</span>
          <span class="font-medium text-gray-900">{run.files_transferred}</span>
        </div>
        {#if run.bytes_transferred !== null}
          <div>
            <span class="text-gray-500">Size:</span>
            <span class="font-medium text-gray-900">
              {(run.bytes_transferred / (1024 * 1024)).toFixed(1)} MB
            </span>
          </div>
        {/if}
        {#if run.error_count > 0}
          <div>
            <span class="text-gray-500">Errors:</span>
            <span class="font-medium text-red-600">{run.error_count}</span>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Log viewer -->
    <div class="flex-1 min-h-[300px] overflow-hidden relative">
      <div class="absolute inset-0">
        <LogViewer {logs} />
      </div>
    </div>

    <!-- Footer -->
    <div class="p-3 border-t bg-gray-50 flex justify-end shrink-0">
      <button onclick={onClose} class="btn btn-secondary text-sm">
        Close
      </button>
    </div>
  </div>
</div>

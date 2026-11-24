import { writable } from 'svelte/store';
import type { FileTask } from '$lib/types';

// Status store for global job status updates
interface StatusUpdate {
  // resync: events were missed, reload everything
  type: 'job_queued' | 'job_started' | 'job_completed' | 'resync';
  run_id?: number;
  job_id?: number;
  timestamp?: string;
}

/** File manager copy/move progress; kept apart from the job events */
export type FileTaskMessage = { type: 'file_task'; task: FileTask } | { type: 'file_tasks_resync' };

const RECONNECT_DELAY_MS = 5000;

function createStatusStore() {
  const { subscribe, update } = writable<StatusUpdate[]>([]);
  let ws: WebSocket | null = null;
  // Number of mounted components that want live updates. Pages connect on
  // mount and disconnect on destroy; when switching pages the new page can
  // mount before the old one is destroyed, so a plain on/off flag would
  // close the socket the new page just asked for.
  let users = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  const fileTaskListeners = new Set<(message: FileTaskMessage) => void>();

  function open() {
    // Authenticated by the session cookie, which the browser sends with the handshake
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const socket = new WebSocket(`${protocol}//${window.location.host}/api/ws/status`);
    ws = socket;

    // Events sent while the socket was down are lost: file tasks reload
    socket.onopen = () => {
      for (const listener of fileTaskListeners) listener({ type: 'file_tasks_resync' });
    };

    socket.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data) as StatusUpdate | FileTaskMessage;
        if (message.type === 'file_task' || message.type === 'file_tasks_resync') {
          for (const listener of fileTaskListeners) listener(message);
          return;
        }
        update((updates) => [...updates.slice(-99), message]);
      } catch {
        // Ignore parse errors
      }
    };

    socket.onclose = () => {
      // A socket closed on purpose (or replaced) must not clear or reconnect the current one
      if (ws !== socket) return;
      ws = null;
      if (users > 0 && !reconnectTimer) {
        reconnectTimer = setTimeout(() => {
          reconnectTimer = null;
          if (users > 0 && !ws) open();
        }, RECONNECT_DELAY_MS);
      }
    };
  }

  function close() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (ws) {
      const socket = ws;
      ws = null;
      socket.close();
    }
  }

  return {
    subscribe,

    connect() {
      users++;
      if (!ws && !reconnectTimer) open();
    },

    disconnect() {
      users = Math.max(0, users - 1);
      if (users === 0) close();
    },

    /** Receive file task messages while connected; returns the unsubscribe */
    onFileTask(listener: (message: FileTaskMessage) => void): () => void {
      fileTaskListeners.add(listener);
      return () => fileTaskListeners.delete(listener);
    },
  };
}

export const statusStore = createStatusStore();

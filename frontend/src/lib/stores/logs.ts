import { writable } from 'svelte/store';
import type { LogEntry } from '../types';
import { api } from '../api';

interface LogMessage {
  run_id: number;
  level: 'debug' | 'info' | 'warning' | 'error';
  message: string;
  source: string;
  timestamp: string;
}

function createLogStore() {
  const { subscribe, update, set } = writable<LogEntry[]>([]);
  let ws: WebSocket | null = null;
  let currentRunId: number | null = null;

  return {
    subscribe,

    async connect(runId: number) {
      if (ws) {
        ws.close();
      }

      currentRunId = runId;
      set([]);

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
          const msg: LogMessage = JSON.parse(event.data);
          const entry: LogEntry = {
            id: Date.now(),
            job_run_id: msg.run_id,
            timestamp: msg.timestamp,
            level: msg.level,
            message: msg.message,
            source: msg.source,
          };
          update((logs) => [...logs, entry]);
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
    },

    disconnect() {
      if (ws) {
        ws.close();
        ws = null;
      }
      currentRunId = null;
    },

    clear() {
      set([]);
    },

    getCurrentRunId() {
      return currentRunId;
    },
  };
}

export const logStore = createLogStore();

// Status store for global job status updates
interface StatusUpdate {
  type: 'job_started' | 'job_completed';
  run_id: number;
  timestamp: string;
}

function createStatusStore() {
  const { subscribe, update } = writable<StatusUpdate[]>([]);
  let ws: WebSocket | null = null;

  return {
    subscribe,

    async connect() {
      if (ws) return;

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
      ws = new WebSocket(`${protocol}//${host}/api/ws/status?token=${encodeURIComponent(token)}`);

      ws.onmessage = (event) => {
        try {
          const status: StatusUpdate = JSON.parse(event.data);
          update((updates) => [...updates.slice(-99), status]);
        } catch {
          // Ignore parse errors
        }
      };

      ws.onclose = () => {
        ws = null;
        // Reconnect after delay
        setTimeout(() => this.connect(), 5000);
      };
    },

    disconnect() {
      if (ws) {
        ws.close();
        ws = null;
      }
    },
  };
}

export const statusStore = createStatusStore();

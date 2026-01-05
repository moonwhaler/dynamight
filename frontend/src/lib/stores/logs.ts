import { writable } from 'svelte/store';
import { api } from '../api';

// Status store for global job status updates
interface StatusUpdate {
  type: 'job_started' | 'job_completed';
  run_id: number;
  timestamp: string;
}

function createStatusStore() {
  const { subscribe, update } = writable<StatusUpdate[]>([]);
  let ws: WebSocket | null = null;
  let intentionalDisconnect = false;

  return {
    subscribe,

    async connect() {
      if (ws) return;

      intentionalDisconnect = false;

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
        // Only reconnect if not intentionally disconnected
        if (!intentionalDisconnect) {
          setTimeout(() => this.connect(), 5000);
        }
      };
    },

    disconnect() {
      intentionalDisconnect = true;
      if (ws) {
        ws.close();
        ws = null;
      }
    },
  };
}

export const statusStore = createStatusStore();

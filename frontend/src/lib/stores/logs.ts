import { writable } from 'svelte/store';

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
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  function connect() {
    // Already connected or connecting
    if (ws) return;

    intentionalDisconnect = false;

    // Authenticated by the session cookie, which the browser sends with the handshake
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    ws = new WebSocket(`${protocol}//${host}/api/ws/status`);

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
      if (!intentionalDisconnect && !reconnectTimer) {
        reconnectTimer = setTimeout(() => {
          reconnectTimer = null;
          connect();
        }, 5000);
      }
    };
  }

  return {
    subscribe,

    connect,

    disconnect() {
      intentionalDisconnect = true;
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (ws) {
        ws.close();
        ws = null;
      }
    },
  };
}

export const statusStore = createStatusStore();

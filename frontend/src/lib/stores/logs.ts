import { writable } from 'svelte/store';

// Status store for global job status updates
interface StatusUpdate {
  type: 'job_started' | 'job_completed';
  run_id: number;
  timestamp: string;
}

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

  function open() {
    // Authenticated by the session cookie, which the browser sends with the handshake
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const socket = new WebSocket(`${protocol}//${window.location.host}/api/ws/status`);
    ws = socket;

    socket.onmessage = (event) => {
      try {
        const status: StatusUpdate = JSON.parse(event.data);
        update((updates) => [...updates.slice(-99), status]);
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
  };
}

export const statusStore = createStatusStore();

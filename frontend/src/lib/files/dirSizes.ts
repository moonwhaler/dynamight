import { API_BASE } from '$lib/api';

export interface DirSizeOptions {
  signal?: AbortSignal;
  /** How often collected sizes are handed over, in ms */
  flushEvery?: number;
  fetchImpl?: typeof fetch;
}

/**
 * Ask the server for folder sizes. They arrive one line at a time; they are
 * collected and handed to `onSizes` in batches, so a big folder does not
 * redraw the list for every subfolder.
 */
export async function streamDirSizes(
  paths: string[],
  onSizes: (sizes: Record<string, number>) => void,
  { signal, flushEvery = 250, fetchImpl = fetch }: DirSizeOptions = {},
): Promise<void> {
  if (paths.length === 0) return;
  const response = await fetchImpl(`${API_BASE}/files/dir-sizes`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paths }),
    signal,
  });
  if (!response.ok || !response.body) return;

  let pending: Record<string, number> = {};
  let timer: ReturnType<typeof setTimeout> | null = null;
  const flush = () => {
    timer = null;
    if (Object.keys(pending).length === 0 || signal?.aborted) return;
    const batch = pending;
    pending = {};
    onSizes(batch);
  };

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffered = '';
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buffered += decoder.decode(value, { stream: true });
      const lines = buffered.split('\n');
      buffered = lines.pop() ?? '';
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const { path, size } = JSON.parse(line) as { path: string; size: number };
          pending[path] = size;
        } catch {
          // Ignore a malformed line
        }
      }
      timer ??= setTimeout(flush, flushEvery);
    }
  } finally {
    if (timer) clearTimeout(timer);
    flush();
  }
}

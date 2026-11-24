import { describe, it, expect, vi } from 'vitest';
import { streamDirSizes } from './dirSizes';

function streamOf(chunks: string[]): Response {
  const encoder = new TextEncoder();
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const c of chunks) controller.enqueue(encoder.encode(c));
      controller.close();
    },
  });
  return new Response(body, { status: 200 });
}

describe('streamDirSizes', () => {
  it('parses lines split across chunks and hands them over', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(streamOf(['{"path":"/a","si', 'ze":10}\n{"path":"/b","size":0}\n']));
    const seen: Record<string, number> = {};
    await streamDirSizes(['/a', '/b'], (s) => Object.assign(seen, s), { fetchImpl, flushEvery: 1000 });
    expect(seen).toEqual({ '/a': 10, '/b': 0 });
    expect(fetchImpl).toHaveBeenCalledOnce();
  });

  it('does nothing without paths', async () => {
    const fetchImpl = vi.fn();
    await streamDirSizes([], () => {}, { fetchImpl });
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { contentRange, fingerprint, secondsLeft, filesFromFolderInput, uploader } from './uploader.svelte';

describe('upload helpers', () => {
  it('builds the chunk range header', () => {
    expect(contentRange(0, 100, 300)).toBe('bytes 0-99/300');
    expect(contentRange(200, 100, 300)).toBe('bytes 200-299/300');
  });

  it('fingerprints a file by name, size and date', () => {
    expect(fingerprint({ name: 'a.mp4', size: 10, lastModified: 5 })).toBe('a.mp4|10|5');
  });

  it('estimates the time left', () => {
    const file = new File([new Uint8Array(1000)], 'x');
    expect(secondsLeft({ file, sent: 400, speed: 100 })).toBe(6);
    expect(secondsLeft({ file, sent: 400, speed: 0 })).toBeNull();
  });

  it('keeps the folder of files from a folder picker', () => {
    const file = new File(['x'], 'b.txt');
    Object.defineProperty(file, 'webkitRelativePath', { value: 'album/2024/b.txt' });
    expect(filesFromFolderInput([file] as unknown as FileList)).toEqual([{ file, relDir: 'album/2024' }]);
  });
});

/** A fake XMLHttpRequest that answers each PUT with the next scripted reply */
type Reply = { status: number; body: unknown } | 'network';
let replies: Reply[] = [];
const sent: { range: string; size: number }[] = [];

class FakeXHR {
  status = 0;
  responseText = '';
  upload = { onprogress: null as ((e: { loaded: number }) => void) | null };
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onabort: (() => void) | null = null;
  #range = '';
  open() {}
  setRequestHeader(_name: string, value: string) {
    this.#range = value;
  }
  abort() {
    this.onabort?.();
  }
  send(body: Blob) {
    sent.push({ range: this.#range, size: body.size });
    const reply = replies.shift() ?? 'network';
    setTimeout(() => {
      if (reply === 'network') return this.onerror?.();
      this.status = reply.status;
      this.responseText = JSON.stringify(reply.body);
      this.onload?.();
    }, 0);
  }
}

describe('uploader', () => {
  beforeEach(() => {
    replies = [];
    sent.length = 0;
    vi.stubGlobal('XMLHttpRequest', FakeXHR);
    vi.useFakeTimers({ toFake: ['setTimeout'] });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    uploader.clearFinished();
  });

  it('sends chunks, resumes after a network error and finishes', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url.endsWith('/files/uploads')) {
        return new Response(JSON.stringify({ id: 'a'.repeat(32), offset: 0, size: 250, chunk_size: 100 }), { status: 200 });
      }
      // Status after the network error: the server got the first chunk
      return new Response(JSON.stringify({ id: 'a'.repeat(32), offset: 100, size: 250 }), { status: 200 });
    });
    vi.stubGlobal('fetch', fetchMock);
    replies = [
      { status: 200, body: { offset: 100, done: false } },
      'network',
      { status: 200, body: { offset: 200, done: false } },
      { status: 200, body: { offset: 250, done: true, path: '/m/x.bin' } },
    ];
    const ended: string[] = [];
    const off = uploader.onFinished((i) => ended.push(i.state));
    uploader.add([{ file: new File([new Uint8Array(250)], 'x.bin'), relDir: '' }], '/m');
    await vi.runAllTimersAsync();
    off();
    expect(ended).toEqual(['done']);
    expect(sent.map((s) => s.range)).toEqual(['bytes 0-99/250', 'bytes 100-199/250', 'bytes 100-199/250', 'bytes 200-249/250']);
    expect(uploader.items[0].path).toBe('/m/x.bin');
  });

  it('waits for a choice when the name is taken', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ code: 'NAME_TAKEN', params: { is_dir: false, size: 3, modified: 1 } }), { status: 409 })),
    );
    uploader.add([{ file: new File(['abc'], 'a.txt'), relDir: '' }], '/m');
    await vi.runAllTimersAsync();
    expect(uploader.conflict?.existing?.size).toBe(3);
    uploader.resolve(uploader.conflict!.key, 'skip', false);
    expect(uploader.items[0].state).toBe('skipped');
    expect(uploader.conflict).toBeNull();
  });
});

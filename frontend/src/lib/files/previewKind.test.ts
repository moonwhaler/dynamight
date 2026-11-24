import { describe, it, expect } from 'vitest';
import { previewKind, audioQueue } from './previewKind';
import type { DirectoryEntry } from '$lib/types';

const e = (name: string, is_dir = false): DirectoryEntry => ({
  name,
  path: `/m/${name}`,
  is_dir,
  size: 1,
  modified: 0,
  extension: name.includes('.') ? name.split('.').pop()!.toLowerCase() : null,
});

describe('previewKind', () => {
  it('maps extensions', () => {
    expect(previewKind(e('IMG.JPG'))).toBe('image');
    expect(previewKind(e('clip.mp4'))).toBe('video');
    expect(previewKind(e('song.flac'))).toBe('audio');
    expect(previewKind(e('doc.pdf'))).toBe('pdf');
    expect(previewKind(e('run.log'))).toBe('text');
    expect(previewKind(e('README'))).toBe('text');
    expect(previewKind(e('disk.iso'))).toBe('other');
    expect(previewKind(e('photos', true))).toBe('other');
  });

  it('queues the audio files in listing order', () => {
    const list = [e('b.mp3'), e('cover.jpg'), e('a.flac'), e('music', true)];
    expect(audioQueue(list).map((x) => x.name)).toEqual(['b.mp3', 'a.flac']);
  });
});

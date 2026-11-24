import { describe, it, expect } from 'vitest';
import { overallProgress, isOpen } from './tasks.svelte';
import type { FileTask } from '$lib/types';

const task = (over: Partial<FileTask>): FileTask => ({
  id: 1,
  revision: 1,
  kind: 'copy',
  sources: [],
  target: '/t',
  state: 'running',
  total_bytes: 0,
  done_bytes: 0,
  total_files: 0,
  done_files: 0,
  current: null,
  conflict: null,
  errors: [],
  failure: null,
  created_at: 0,
  finished_at: null,
  ...over,
});

describe('overallProgress', () => {
  it('weighs open tasks by bytes', () => {
    expect(overallProgress([task({ total_bytes: 100, done_bytes: 50 }), task({ total_bytes: 300, done_bytes: 0 })])).toBe(0.125);
  });

  it('ignores finished tasks and falls back to file counts', () => {
    expect(overallProgress([task({ state: 'done', total_bytes: 10, done_bytes: 10 }), task({ total_files: 4, done_files: 1 })])).toBe(0.25);
    expect(overallProgress([])).toBe(0);
  });

  it('knows which tasks are still open', () => {
    expect(isOpen(task({ state: 'conflict' }))).toBe(true);
    expect(isOpen(task({ state: 'cancelled' }))).toBe(false);
  });
});

import { describe, it, expect } from 'vitest';
import { sortEntries } from './fileSort';
import type { DirectoryEntry } from '$lib/types';

const e = (name: string, is_dir = false, size: number | null = null, modified: number | null = null): DirectoryEntry => ({
  name,
  path: `/x/${name}`,
  is_dir,
  is_symlink: false,
  size,
  modified,
  extension: null,
});

describe('sortEntries', () => {
  it('puts folders first and sorts numbers naturally', () => {
    const names = sortEntries([e('file10'), e('b', true), e('file2'), e('A', true)], 'name', 'asc').map((x) => x.name);
    expect(names).toEqual(['A', 'b', 'file2', 'file10']);
  });

  it('sorts by size descending, unknown sizes last, ties by name', () => {
    const names = sortEntries([e('a', false, 5), e('c', false, 10), e('b', false, 5), e('n', false, null)], 'size', 'desc').map(
      (x) => x.name,
    );
    expect(names).toEqual(['c', 'a', 'b', 'n']);
  });

  it('sorts by date', () => {
    const names = sortEntries([e('new', false, 1, 300), e('old', false, 1, 100)], 'modified', 'asc').map((x) => x.name);
    expect(names).toEqual(['old', 'new']);
  });
});

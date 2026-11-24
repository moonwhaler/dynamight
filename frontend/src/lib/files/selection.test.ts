import { describe, it, expect } from 'vitest';
import { rangeOf, selectionState } from './selection';

const paths = ['/a', '/b', '/c', '/d'];

describe('rangeOf', () => {
  it('takes the rows between anchor and click, in either direction', () => {
    expect(rangeOf(paths, 1, 3)).toEqual(['/b', '/c', '/d']);
    expect(rangeOf(paths, 2, 0)).toEqual(['/a', '/b', '/c']);
    expect(rangeOf(paths, 2, 2)).toEqual(['/c']);
  });

  it('clamps to the list', () => {
    expect(rangeOf(paths, -3, 10)).toEqual(paths);
    expect(rangeOf([], 0, 2)).toEqual([]);
  });
});

describe('selectionState', () => {
  it('reports none, some or all', () => {
    expect(selectionState(paths, new Set())).toBe('none');
    expect(selectionState(paths, new Set(['/a']))).toBe('some');
    expect(selectionState(paths, new Set(paths))).toBe('all');
    // Selected items of another folder do not count
    expect(selectionState(paths, new Set(['/x']))).toBe('none');
  });
});

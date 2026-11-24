/** Paths from `anchor` to `index` (either order), for Shift-click */
export function rangeOf(paths: string[], anchor: number, index: number): string[] {
  if (paths.length === 0) return [];
  const from = Math.max(0, Math.min(anchor, index));
  const to = Math.min(paths.length - 1, Math.max(anchor, index));
  return paths.slice(from, to + 1);
}

/** Header checkbox state for a selection within a listing */
export function selectionState(paths: string[], selected: Set<string>): 'none' | 'some' | 'all' {
  let count = 0;
  for (const p of paths) if (selected.has(p)) count++;
  if (count === 0) return 'none';
  return count === paths.length ? 'all' : 'some';
}

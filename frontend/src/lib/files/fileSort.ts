import type { DirectoryEntry } from '$lib/types';

export type SortField = 'name' | 'size' | 'modified';
export type SortOrder = 'asc' | 'desc';

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

/** Folders first, then by field; names compare like people read them ("file2" before "file10"). */
export function sortEntries(entries: DirectoryEntry[], field: SortField, order: SortOrder): DirectoryEntry[] {
  const dir = order === 'asc' ? 1 : -1;
  return [...entries].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    let cmp = 0;
    if (field === 'size') cmp = (a.size ?? -1) - (b.size ?? -1);
    else if (field === 'modified') cmp = (a.modified ?? 0) - (b.modified ?? 0);
    // Equal sizes or dates fall back to the name, so the order is stable
    if (cmp === 0) return collator.compare(a.name, b.name) * (field === 'name' ? dir : 1);
    return cmp * dir;
  });
}

/**
 * Shorten a path from the middle so its end stays readable:
 * "/mnt/backup/photos/2026" -> "/mnt/…/photos/2026".
 */
export function shortenPath(p: string, limit = 42): string {
  if (p.length <= limit) return p;
  const parts = p.split('/');
  let tail = parts.pop() ?? '';
  while (parts.length > 1 && tail.length + parts[parts.length - 1].length + 6 < limit) {
    tail = `${parts.pop()}/${tail}`;
  }
  const head = p.startsWith('/') ? `/${parts[1] ?? ''}` : (parts[0] ?? '');
  const out = `${head}/…/${tail}`;
  return out.length < p.length ? out : `…${p.slice(-(limit - 1))}`;
}

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
const K = 1024;

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return '-';
  if (bytes === 0) return '0 B';
  const i = Math.floor(Math.log(bytes) / Math.log(K));
  return `${parseFloat((bytes / Math.pow(K, i)).toFixed(1))} ${BYTE_UNITS[i]}`;
}

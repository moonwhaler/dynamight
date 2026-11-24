import type { DirectoryEntry } from '$lib/types';

export type PreviewKind = 'image' | 'video' | 'audio' | 'pdf' | 'text' | 'other';

const IMAGE = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'avif', 'svg', 'bmp', 'ico']);
const VIDEO = new Set(['mp4', 'm4v', 'webm', 'mov', 'mkv', 'ogv', 'avi']);
const AUDIO = new Set(['mp3', 'flac', 'ogg', 'oga', 'opus', 'wav', 'm4a', 'aac']);
const TEXT = new Set([
  'txt', 'log', 'md', 'markdown', 'csv', 'tsv', 'json', 'jsonl', 'ndjson', 'xml', 'yml', 'yaml', 'toml', 'ini',
  'conf', 'cfg', 'env', 'properties', 'html', 'htm', 'css', 'scss', 'js', 'mjs', 'ts', 'tsx', 'jsx', 'svelte',
  'vue', 'sh', 'bash', 'zsh', 'py', 'rb', 'rs', 'go', 'java', 'kt', 'c', 'h', 'cpp', 'hpp', 'cs', 'php', 'sql',
  'lua', 'pl', 'r', 'swift', 'gradle', 'dockerfile', 'gitignore', 'service', 'timer', 'nfo', 'srt', 'vtt', 'sub',
]);

/** How a file is shown in the preview, by its extension */
export function previewKind(entry: Pick<DirectoryEntry, 'is_dir' | 'extension' | 'name'>): PreviewKind {
  if (entry.is_dir) return 'other';
  const ext = (entry.extension ?? '').toLowerCase();
  if (IMAGE.has(ext)) return 'image';
  if (VIDEO.has(ext)) return 'video';
  if (AUDIO.has(ext)) return 'audio';
  if (ext === 'pdf') return 'pdf';
  // No extension (README, Makefile, logs) is worth a look as text
  if (TEXT.has(ext) || !ext) return 'text';
  return 'other';
}

/** Files of a folder that play one after another */
export function audioQueue(entries: DirectoryEntry[]): DirectoryEntry[] {
  return entries.filter((e) => previewKind(e) === 'audio');
}

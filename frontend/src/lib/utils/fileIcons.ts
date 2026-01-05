// File extension to icon and color mapping for the file browser

export interface FileIconInfo {
  icon: 'folder' | 'image' | 'document' | 'archive' | 'code' | 'video' | 'audio' | 'file';
  color: string;
}

const extensionMap: Record<string, FileIconInfo> = {
  // Images
  jpg: { icon: 'image', color: 'text-pink-500' },
  jpeg: { icon: 'image', color: 'text-pink-500' },
  png: { icon: 'image', color: 'text-pink-500' },
  gif: { icon: 'image', color: 'text-pink-500' },
  webp: { icon: 'image', color: 'text-pink-500' },
  svg: { icon: 'image', color: 'text-pink-500' },
  ico: { icon: 'image', color: 'text-pink-500' },
  bmp: { icon: 'image', color: 'text-pink-500' },

  // Documents
  pdf: { icon: 'document', color: 'text-red-500' },
  doc: { icon: 'document', color: 'text-blue-500' },
  docx: { icon: 'document', color: 'text-blue-500' },
  xls: { icon: 'document', color: 'text-green-600' },
  xlsx: { icon: 'document', color: 'text-green-600' },
  ppt: { icon: 'document', color: 'text-orange-500' },
  pptx: { icon: 'document', color: 'text-orange-500' },
  odt: { icon: 'document', color: 'text-blue-500' },
  ods: { icon: 'document', color: 'text-green-600' },
  txt: { icon: 'document', color: 'text-gray-500' },
  rtf: { icon: 'document', color: 'text-gray-500' },
  md: { icon: 'document', color: 'text-gray-600' },

  // Archives
  zip: { icon: 'archive', color: 'text-yellow-600' },
  rar: { icon: 'archive', color: 'text-yellow-600' },
  '7z': { icon: 'archive', color: 'text-yellow-600' },
  tar: { icon: 'archive', color: 'text-yellow-600' },
  gz: { icon: 'archive', color: 'text-yellow-600' },
  bz2: { icon: 'archive', color: 'text-yellow-600' },
  xz: { icon: 'archive', color: 'text-yellow-600' },

  // Code
  js: { icon: 'code', color: 'text-yellow-400' },
  ts: { icon: 'code', color: 'text-blue-400' },
  jsx: { icon: 'code', color: 'text-cyan-400' },
  tsx: { icon: 'code', color: 'text-blue-400' },
  py: { icon: 'code', color: 'text-green-400' },
  rs: { icon: 'code', color: 'text-orange-400' },
  go: { icon: 'code', color: 'text-cyan-400' },
  java: { icon: 'code', color: 'text-red-400' },
  c: { icon: 'code', color: 'text-blue-400' },
  cpp: { icon: 'code', color: 'text-blue-500' },
  h: { icon: 'code', color: 'text-purple-400' },
  hpp: { icon: 'code', color: 'text-purple-500' },
  css: { icon: 'code', color: 'text-blue-400' },
  scss: { icon: 'code', color: 'text-pink-400' },
  html: { icon: 'code', color: 'text-orange-400' },
  json: { icon: 'code', color: 'text-yellow-400' },
  xml: { icon: 'code', color: 'text-orange-400' },
  yaml: { icon: 'code', color: 'text-red-400' },
  yml: { icon: 'code', color: 'text-red-400' },
  toml: { icon: 'code', color: 'text-gray-500' },
  sh: { icon: 'code', color: 'text-green-500' },
  bash: { icon: 'code', color: 'text-green-500' },
  sql: { icon: 'code', color: 'text-blue-500' },
  svelte: { icon: 'code', color: 'text-orange-500' },

  // Video
  mp4: { icon: 'video', color: 'text-purple-500' },
  mkv: { icon: 'video', color: 'text-purple-500' },
  avi: { icon: 'video', color: 'text-purple-500' },
  mov: { icon: 'video', color: 'text-purple-500' },
  wmv: { icon: 'video', color: 'text-purple-500' },
  webm: { icon: 'video', color: 'text-purple-500' },

  // Audio
  mp3: { icon: 'audio', color: 'text-green-500' },
  wav: { icon: 'audio', color: 'text-green-500' },
  flac: { icon: 'audio', color: 'text-green-500' },
  ogg: { icon: 'audio', color: 'text-green-500' },
  m4a: { icon: 'audio', color: 'text-green-500' },
  aac: { icon: 'audio', color: 'text-green-500' },
};

const defaultIcon: FileIconInfo = { icon: 'file', color: 'text-gray-400' };
const folderIcon: FileIconInfo = { icon: 'folder', color: 'text-primary-500' };

export function getFileIcon(extension: string | null, isDir: boolean): FileIconInfo {
  if (isDir) {
    return folderIcon;
  }

  if (!extension) {
    return defaultIcon;
  }

  return extensionMap[extension.toLowerCase()] ?? defaultIcon;
}

export function formatFileSize(bytes: number | null): string {
  if (bytes === null || bytes === undefined) return '-';
  if (bytes === 0) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${units[i]}`;
}

export function formatDate(timestamp: number | null): string {
  if (timestamp === null || timestamp === undefined) return '-';

  const date = new Date(timestamp * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  } else if (diffDays === 1) {
    return 'Yesterday';
  } else if (diffDays < 7) {
    return date.toLocaleDateString(undefined, { weekday: 'short' });
  } else {
    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
}

import { describe, it, expect } from 'vitest';
import { getFileIcon, formatFileType, formatFileSize, formatDate } from './fileIcons';

describe('getFileIcon', () => {
  it('returns folder icon for directories', () => {
    const result = getFileIcon(null, true);
    expect(result.icon).toBe('folder');
  });

  it('returns default for null extension', () => {
    const result = getFileIcon(null, false);
    expect(result.icon).toBe('file');
  });

  it('returns image icon for jpg', () => {
    expect(getFileIcon('jpg', false).icon).toBe('image');
  });

  it('returns document icon for pdf', () => {
    expect(getFileIcon('pdf', false).icon).toBe('document');
  });

  it('returns archive icon for zip', () => {
    expect(getFileIcon('zip', false).icon).toBe('archive');
  });

  it('returns code icon for ts', () => {
    expect(getFileIcon('ts', false).icon).toBe('code');
  });

  it('returns video icon for mp4', () => {
    expect(getFileIcon('mp4', false).icon).toBe('video');
  });

  it('returns audio icon for mp3', () => {
    expect(getFileIcon('mp3', false).icon).toBe('audio');
  });

  it('is case insensitive', () => {
    expect(getFileIcon('JPG', false).icon).toBe('image');
    expect(getFileIcon('PDF', false).icon).toBe('document');
  });

  it('returns default for unknown extension', () => {
    expect(getFileIcon('xyz', false).icon).toBe('file');
  });
});

describe('formatFileType', () => {
  it('returns folder label for directories', () => {
    const result = formatFileType(null, true);
    expect(result).toBe('filebrowser_type_folder');
  });

  it('returns extension with dot', () => {
    expect(formatFileType('pdf', false)).toBe('.pdf');
  });

  it('returns file label for no extension', () => {
    expect(formatFileType(null, false)).toBe('filebrowser_type_file');
  });
});

describe('formatFileSize', () => {
  it('delegates to formatBytes', () => {
    expect(formatFileSize(1024)).toBe('1 KB');
    expect(formatFileSize(null)).toBe('-');
  });
});

describe('formatDate', () => {
  it('returns dash for null', () => {
    expect(formatDate(null)).toBe('-');
  });

  it('returns time for today', () => {
    const now = Math.floor(Date.now() / 1000);
    const result = formatDate(now);
    // Should contain a colon (time format like "14:30")
    expect(result).toMatch(/:/);
  });

  it('returns yesterday label', () => {
    const yesterday = Math.floor(Date.now() / 1000) - 86400;
    expect(formatDate(yesterday)).toBe('filebrowser_date_yesterday');
  });

  it('returns weekday for recent days', () => {
    const threeDaysAgo = Math.floor(Date.now() / 1000) - 3 * 86400;
    const result = formatDate(threeDaysAgo);
    // Should be a short weekday like "Mon", "Tue", etc.
    expect(result.length).toBeGreaterThan(0);
    expect(result).not.toBe('-');
  });

  it('returns short date for older dates', () => {
    const twoWeeksAgo = Math.floor(Date.now() / 1000) - 14 * 86400;
    const result = formatDate(twoWeeksAgo);
    expect(result.length).toBeGreaterThan(0);
    expect(result).not.toBe('-');
  });
});

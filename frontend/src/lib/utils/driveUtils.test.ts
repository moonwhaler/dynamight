import { describe, it, expect } from 'vitest';
import { getDriveDisplayName, getDriveFullName, shouldShowDriveTooltip } from './driveUtils';
import type { UsbDrive } from '$lib/types';

function makeDrive(overrides: Partial<UsbDrive> = {}): UsbDrive {
  return {
    name: 'sda1',
    label: '',
    model: '',
    uuid: 'test-uuid',
    size: 0,
    mounted: false,
    mount_point: null,
    ...overrides,
  } as UsbDrive;
}

describe('getDriveDisplayName', () => {
  it('prefers label', () => {
    const drive = makeDrive({ label: 'My USB', model: 'SanDisk', name: 'sda1' });
    expect(getDriveDisplayName(drive)).toBe('My USB');
  });

  it('falls back to model', () => {
    const drive = makeDrive({ label: '', model: 'SanDisk Ultra', name: 'sda1' });
    expect(getDriveDisplayName(drive)).toBe('SanDisk Ultra');
  });

  it('falls back to name', () => {
    const drive = makeDrive({ label: '', model: '', name: 'sda1' });
    expect(getDriveDisplayName(drive)).toBe('sda1');
  });

  it('truncates with ellipsis', () => {
    const drive = makeDrive({ label: 'My Very Long USB Drive Name' });
    const result = getDriveDisplayName(drive, 10);
    expect(result.length).toBe(10);
    expect(result).toContain('\u2026');
  });

  it('does not truncate when within limit', () => {
    const drive = makeDrive({ label: 'Short' });
    expect(getDriveDisplayName(drive, 10)).toBe('Short');
  });
});

describe('getDriveFullName', () => {
  it('returns full name without truncation', () => {
    const drive = makeDrive({ label: 'My Very Long USB Drive Name' });
    expect(getDriveFullName(drive)).toBe('My Very Long USB Drive Name');
  });
});

describe('shouldShowDriveTooltip', () => {
  it('returns true when name exceeds max', () => {
    const drive = makeDrive({ label: 'Long Name Here' });
    expect(shouldShowDriveTooltip(drive, 5)).toBe(true);
  });

  it('returns false when name fits', () => {
    const drive = makeDrive({ label: 'Short' });
    expect(shouldShowDriveTooltip(drive, 20)).toBe(false);
  });
});

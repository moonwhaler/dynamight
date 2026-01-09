import type { UsbDrive } from '$lib/types';

/**
 * Get a user-friendly display name for a USB drive.
 * Priority: label > model > device name (e.g., sda1)
 *
 * @param drive - The USB drive object
 * @param maxLength - Optional maximum length for truncation (includes ellipsis)
 * @returns The display name, optionally truncated
 */
export function getDriveDisplayName(drive: UsbDrive, maxLength?: number): string {
  // Priority: label > model > name
  const displayName = drive.label || drive.model || drive.name;

  if (maxLength && displayName.length > maxLength) {
    return displayName.slice(0, maxLength - 1) + '\u2026'; // Unicode ellipsis
  }

  return displayName;
}

/**
 * Get the full display name without truncation (for tooltips)
 */
export function getDriveFullName(drive: UsbDrive): string {
  return drive.label || drive.model || drive.name;
}

/**
 * Check if the drive display name should show a tooltip
 * (i.e., if it would be truncated at the given length)
 */
export function shouldShowDriveTooltip(drive: UsbDrive, maxLength: number): boolean {
  const fullName = getDriveFullName(drive);
  return fullName.length > maxLength;
}

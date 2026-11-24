import { describe, it, expect } from 'vitest';
import { formatStatus, getStatusBadgeClass } from './status';

describe('formatStatus', () => {
  it('maps completed', () => {
    expect(formatStatus('completed')).toBe('history_status_completed');
  });

  it('maps running', () => {
    expect(formatStatus('running')).toBe('history_status_running');
  });

  it('maps failed', () => {
    expect(formatStatus('failed')).toBe('history_status_failed');
  });

  it('maps cancelled', () => {
    expect(formatStatus('cancelled')).toBe('history_status_cancelled');
  });

  it('maps pending', () => {
    expect(formatStatus('pending')).toBe('history_status_pending');
  });

  it('returns unknown status as-is', () => {
    expect(formatStatus('unknown')).toBe('unknown');
  });
});

describe('getStatusBadgeClass', () => {
  it('returns badge-success for completed', () => {
    expect(getStatusBadgeClass('completed')).toBe('badge-success');
  });

  it('returns badge-info for running', () => {
    expect(getStatusBadgeClass('running')).toBe('badge-info');
  });

  it('returns badge-error for failed', () => {
    expect(getStatusBadgeClass('failed')).toBe('badge-error');
  });

  it('returns badge-warning for cancelled', () => {
    expect(getStatusBadgeClass('cancelled')).toBe('badge-warning');
  });

  it('returns badge-gray for unknown', () => {
    expect(getStatusBadgeClass('other')).toBe('badge-gray');
  });
});

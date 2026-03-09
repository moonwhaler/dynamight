import { describe, it, expect } from 'vitest';
import { formatStatus, getStatusIndicator, getStatusBadgeClass } from './status';

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

describe('getStatusIndicator', () => {
  it('returns green for completed', () => {
    const result = getStatusIndicator('completed');
    expect(result.color).toBe('bg-green-500');
  });

  it('returns red for failed', () => {
    const result = getStatusIndicator('failed');
    expect(result.color).toBe('bg-red-500');
  });

  it('returns blue for running', () => {
    const result = getStatusIndicator('running');
    expect(result.color).toBe('bg-blue-500');
  });

  it('returns orange for cancelled', () => {
    const result = getStatusIndicator('cancelled');
    expect(result.color).toBe('bg-orange-500');
  });

  it('returns yellow for pending', () => {
    const result = getStatusIndicator('pending');
    expect(result.color).toBe('bg-yellow-500');
  });

  it('returns empty for null', () => {
    const result = getStatusIndicator(null);
    expect(result.color).toBe('');
    expect(result.label).toBe('');
  });

  it('returns empty for undefined', () => {
    const result = getStatusIndicator(undefined);
    expect(result.color).toBe('');
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

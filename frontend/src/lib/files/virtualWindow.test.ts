import { describe, it, expect } from 'vitest';
import { virtualWindow } from './virtualWindow';

describe('virtualWindow', () => {
  it('renders everything for a short list', () => {
    expect(virtualWindow(0, 400, 40, 5)).toEqual({ start: 0, end: 5, padTop: 0, padBottom: 0 });
  });

  it('keeps overscan rows around the visible ones', () => {
    // Rows 100..110 visible, 20 rows extra on each side
    const w = virtualWindow(4000, 400, 40, 10_000);
    expect(w.start).toBe(80);
    expect(w.end).toBe(131);
    expect(w.padTop).toBe(80 * 40);
    expect(w.padBottom).toBe((10_000 - 131) * 40);
  });

  it('clamps at the end of the list', () => {
    const w = virtualWindow(1_000_000, 400, 40, 100);
    expect(w.end).toBe(100);
    expect(w.start).toBeLessThanOrEqual(w.end);
    expect(w.padBottom).toBe(0);
  });

  it('handles an empty list', () => {
    expect(virtualWindow(0, 400, 40, 0)).toEqual({ start: 0, end: 0, padTop: 0, padBottom: 0 });
  });
});

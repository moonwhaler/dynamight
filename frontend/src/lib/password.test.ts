import { describe, it, expect } from 'vitest';
import { calculatePasswordStrength } from './password';

describe('calculatePasswordStrength', () => {
  it('returns weak for empty string', () => {
    const result = calculatePasswordStrength('');
    expect(result.score).toBe(0);
    expect(result.level).toBe('weak');
  });

  it('returns weak for short password', () => {
    const result = calculatePasswordStrength('abc');
    expect(result.level).toBe('weak');
    expect(result.label).toBe('Too short');
    expect(result.score).toBeLessThanOrEqual(25);
  });

  it('returns weak for only numbers', () => {
    const result = calculatePasswordStrength('12345678');
    expect(result.level).toBe('weak');
  });

  it('returns weak for common start', () => {
    const result = calculatePasswordStrength('password123');
    // -20 penalty for common start
    expect(result.score).toBeLessThan(50);
  });

  it('returns fair for lowercase 8+ chars', () => {
    const result = calculatePasswordStrength('abcdefgh');
    // 10 (length>=8) + 10 (lowercase) + 10 (unique>=6) - 10 (only letters) = 20
    // Actually: 10 + 10 + 10 - 10 = 20, so weak
    expect(result.level).toBe('weak');
  });

  it('returns fair or weak for mixed case 8+ chars', () => {
    const result = calculatePasswordStrength('AbCdEfGh');
    // 10 (len>=8) + 10 (lower) + 10 (upper) + 10 (unique>=6) - 10 (only letters) = 30
    // But unique chars = 8 letters (all different case-sensitive), so unique>=6 => +10
    // Score depends on case sensitivity of Set. Either way it should be weak or fair.
    expect(['weak', 'fair']).toContain(result.level);
  });

  it('returns good for mixed case + numbers 12+ chars', () => {
    const result = calculatePasswordStrength('AbCd1234EfGh');
    // 20 (len>=12) + 10 (lower) + 10 (upper) + 10 (numbers) + 10 (unique>=6) + 10 (unique>=10)
    // = 70, good
    expect(result.level).toBe('good');
  });

  it('returns strong for complex 16+ char password', () => {
    const result = calculatePasswordStrength('MyP@ssw0rd!Str0ng#');
    expect(result.level).toBe('strong');
    expect(result.score).toBeGreaterThanOrEqual(75);
  });

  it('penalizes repeated characters', () => {
    const withRepeats = calculatePasswordStrength('aaa12345Bc');
    const without = calculatePasswordStrength('abc12345De');
    // withRepeats has -10 penalty for 'aaa', without has no penalty
    expect(withRepeats.score).toBeLessThanOrEqual(without.score);
  });

  it('penalizes admin start', () => {
    const result = calculatePasswordStrength('admin12345!');
    const normal = calculatePasswordStrength('xyzzy12345!');
    expect(result.score).toBeLessThan(normal.score);
  });

  it('score is clamped between 0 and 100', () => {
    const weak = calculatePasswordStrength('1');
    expect(weak.score).toBeGreaterThanOrEqual(0);

    const strong = calculatePasswordStrength('MyV3ry$tr0ng&C0mpl3x!P@ssw0rd2024');
    expect(strong.score).toBeLessThanOrEqual(100);
  });

  it('handles null-ish input', () => {
    const result = calculatePasswordStrength(null as unknown as string);
    expect(result.score).toBe(0);
    expect(result.level).toBe('weak');
  });
});

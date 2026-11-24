import { describe, it, expect } from 'vitest';
import en from '../../messages/en.json';
import de from '../../messages/de.json';
import * as mock from './paraglide-messages';

const keys = (messages: Record<string, unknown>) => Object.keys(messages).filter((k) => !k.startsWith('$'));

describe('paraglide message mock', () => {
  it('has a function for every message', () => {
    const exported = new Set(Object.keys(mock));
    expect(keys(en).filter((k) => !exported.has(k))).toEqual([]);
  });

  it('en and de define the same messages', () => {
    expect(keys(de).sort()).toEqual(keys(en).sort());
  });

  it('returns the key, with parameters when given', () => {
    expect(mock.common_save()).toBe('common_save');
    expect((mock.common_save as (p: Record<string, unknown>) => string)({ n: 2 })).toBe('common_save(n=2)');
  });
});

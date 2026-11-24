import { describe, it, expect } from 'vitest';
import { placeOf, parentOf } from './browser.svelte';

describe('placeOf', () => {
  it('picks the longest place holding the path', () => {
    expect(placeOf('/mnt/USB2/photos', ['/mnt', '/mnt/USB2', '/source'])).toBe('/mnt/USB2');
    expect(placeOf('/mnt/other', ['/mnt', '/mnt/USB2'])).toBe('/mnt');
    expect(placeOf('/mnt/USB2', ['/mnt/USB2/'])).toBe('/mnt/USB2');
  });

  it('compares whole path segments', () => {
    expect(placeOf('/mnt/USB20/x', ['/mnt/USB2'])).toBeNull();
  });
});

describe('parentOf', () => {
  it('returns the folder above', () => {
    expect(parentOf('/mnt/a/b')).toBe('/mnt/a');
    expect(parentOf('/mnt')).toBe('/');
    expect(parentOf('/mnt/a/')).toBe('/mnt');
  });
});

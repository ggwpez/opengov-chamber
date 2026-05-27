import { describe, it, expect } from 'vitest';
import { shorten, parseAddresses, isAddress, isHash32, callHashFromHex } from './format';

describe('isAddress', () => {
  it('accepts a 20-byte 0x address (any case)', () => {
    expect(isAddress('0x' + '1'.repeat(40))).toBe(true);
    expect(isAddress('0xAbCdef0000000000000000000000000000000001')).toBe(true);
  });
  it('rejects wrong length / missing prefix / non-hex', () => {
    expect(isAddress('0x' + '1'.repeat(39))).toBe(false);
    expect(isAddress('1'.repeat(40))).toBe(false);
    expect(isAddress('0x' + 'z'.repeat(40))).toBe(false);
  });
});

describe('isHash32', () => {
  it('accepts a 32-byte 0x hash', () => {
    expect(isHash32('0x' + 'a'.repeat(64))).toBe(true);
  });
  it('rejects a 20-byte address', () => {
    expect(isHash32('0x' + 'a'.repeat(40))).toBe(false);
  });
});

describe('parseAddresses', () => {
  it('splits on newlines, commas and spaces, trimming blanks', () => {
    const raw = `0x1111111111111111111111111111111111111111,
      0x2222222222222222222222222222222222222222   0x3333333333333333333333333333333333333333`;
    expect(parseAddresses(raw)).toEqual([
      '0x1111111111111111111111111111111111111111',
      '0x2222222222222222222222222222222222222222',
      '0x3333333333333333333333333333333333333333',
    ]);
  });
  it('returns an empty list for blank input', () => {
    expect(parseAddresses('   \n  ')).toEqual([]);
  });
});

describe('shorten', () => {
  it('elides the middle of a long 0x value', () => {
    expect(shorten('0x' + 'ab'.repeat(20))).toBe('0xababab…abab');
  });
  it('leaves short or non-0x values untouched', () => {
    expect(shorten('0x1234')).toBe('0x1234');
    expect(shorten('not-a-hex-string')).toBe('not-a-hex-string');
  });
  it('honours custom head/tail lengths', () => {
    expect(shorten('0x' + 'cd'.repeat(20), 5, 4)).toBe('0xcdcdc…cdcd');
  });
});

describe('callHashFromHex', () => {
  // blake2-256 of the empty byte string is a known constant; an empty SCALE call
  // is degenerate but pins the digest wiring.
  it('derives blake2-256 hash + byte length from call hex (with 0x)', () => {
    const { callHash, callLen } = callHashFromHex('0x0a0b0c');
    expect(callLen).toBe(3);
    expect(callHash).toMatch(/^0x[0-9a-f]{64}$/);
  });
  it('accepts hex without the 0x prefix and counts bytes', () => {
    expect(callHashFromHex('deadbeef').callLen).toBe(4);
  });
  it('is stable for the same input', () => {
    expect(callHashFromHex('0xdeadbeef').callHash).toBe(callHashFromHex('deadbeef').callHash);
  });
  it('rejects odd-length, non-hex, or empty input', () => {
    expect(() => callHashFromHex('0xabc')).toThrow();
    expect(() => callHashFromHex('0xzz')).toThrow();
    expect(() => callHashFromHex('0x')).toThrow();
  });
});

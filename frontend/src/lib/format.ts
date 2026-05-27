import { blake2b } from 'blakejs';

/** Shorten a 0x address/hash for display: 0x1234…cdef. */
export function shorten(value: string, head = 6, tail = 4): string {
  if (!value.startsWith('0x') || value.length <= head + tail + 2) return value;
  return `${value.slice(0, head + 2)}…${value.slice(-tail)}`;
}

/** Parse a textarea of addresses (newline/comma/space separated) into a list. */
export function parseAddresses(raw: string): `0x${string}`[] {
  return raw
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean) as `0x${string}`[];
}

export function isAddress(value: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(value);
}

export function isHash32(value: string): boolean {
  return /^0x[0-9a-fA-F]{64}$/.test(value);
}

/**
 * Compute the Substrate preimage hash (blake2-256) and byte length of a SCALE-
 * encoded runtime call, given its hex. This is what `propose()` wants as
 * `callHash` + `callLen`. The preimage itself still has to be noted on Asset Hub
 * (via PAPI / polkadot-js) for a referendum to resolve it — this only derives
 * the identifiers.
 */
export function callHashFromHex(hex: string): { callHash: `0x${string}`; callLen: number } {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (clean.length === 0 || clean.length % 2 !== 0 || /[^0-9a-fA-F]/.test(clean)) {
    throw new Error('Not valid hex');
  }
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  const digest = blake2b(bytes, undefined, 32);
  const callHash =
    '0x' + Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
  return { callHash: callHash as `0x${string}`, callLen: bytes.length };
}

import { keccak256, hexToBytes } from 'viem';
import type { Proposal } from './abi';

/**
 * Recompute a proposal's storage key, mirroring `proposal_key` in
 * `../../../contract/src/lib.rs` BYTE-FOR-BYTE. `allProposals()` returns the
 * structs but not their keys, yet `approve()` / `finalize()` / `proposal()`
 * are all keyed by this hash — so we derive it client-side.
 *
 * Layout (then keccak256):
 *   b"Proposal:"
 *   creator            : [u8; 20]              (SCALE fixed array = raw bytes)
 *   callHash           : [u8; 32]              (raw bytes)
 *   Compact<u32>(approvers.len())
 *   approvers[i]       : [u8; 20] each         (raw bytes, in stored order)
 *   minApprovers       : Compact(32) ++ [u8; 32] little-endian
 *                        (the Rust encodes `U256::as_le_bytes()`, a *slice*, so
 *                         SCALE prepends a compact length — always 0x80 for U256)
 *   callLen            : u32 little-endian      (4 bytes)
 *   enactmentDelay     : u32 little-endian      (4 bytes)
 *
 * NOTE: keep in lockstep with the Rust. A mismatch silently produces a key that
 * resolves to no proposal, so approve/finalize would revert. Worth a parity
 * test against the contract if this is ever changed.
 */
export function proposalKey(p: Proposal): `0x${string}` {
  const parts: Uint8Array[] = [];

  parts.push(new TextEncoder().encode('Proposal:'));
  parts.push(hexToBytes(p.creator)); // 20 bytes
  parts.push(hexToBytes(p.callHash)); // 32 bytes
  parts.push(compactU32(p.approvers.length));
  for (const a of p.approvers) parts.push(hexToBytes(a)); // 20 bytes each
  parts.push(compactU32(32)); // length prefix for the 32-byte as_le_bytes() slice
  parts.push(u256le(p.minApprovers)); // 32 bytes LE
  parts.push(u32le(p.callLen));
  parts.push(u32le(p.enactmentDelay));

  return keccak256(concat(parts));
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

/** SCALE `Compact<u32>` encoding (covers values up to 2^30 - 1). */
function compactU32(value: number): Uint8Array {
  if (value < 0) throw new Error('compactU32: negative');
  if (value < 0x40) return Uint8Array.of(value << 2); // single-byte mode
  if (value < 0x4000) {
    const v = value * 4 + 0b01; // two-byte mode
    return Uint8Array.of(v & 0xff, (v >>> 8) & 0xff);
  }
  if (value < 0x40000000) {
    const v = value * 4 + 0b10; // four-byte mode
    return Uint8Array.of(v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff);
  }
  throw new Error('compactU32: value too large for this encoder');
}

function u32le(value: number): Uint8Array {
  return Uint8Array.of(value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff);
}

function u256le(value: bigint): Uint8Array {
  const out = new Uint8Array(32);
  let v = value;
  for (let i = 0; i < 32; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

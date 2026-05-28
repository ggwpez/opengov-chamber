import { bytesToHex, hexToBytes } from 'viem';
import { DispatchTimeKind, ProposalStatus, Track, type Proposal } from './abi';

/**
 * Packed binary codec for a stored {@link Proposal}, mirroring
 * `contract/src/codec.rs` byte-for-byte. Solidity ABI pads every head field
 * to 32 bytes, which collides with pallet-revive's 416-byte storage cap at
 * just N+M = 2 entries; this layout drops the padding so the same proposal
 * fits in `67 + 20·(N+M)` bytes (N+M ≤ 17 within the cap).
 *
 * The Solidity ABI is still what `propose` / `proposal` / `allProposals`
 * speak on the wire — this codec only describes what the contract puts into
 * `set_storage`. The frontend doesn't decode storage directly today; this
 * lives here as a parity reference (asserted by the parity vector below) and
 * as runway for a future packed-return path.
 *
 * Wire layout (little-endian, no padding):
 *
 *  off  size  field
 *    0   1    version (= VERSION)
 *    1   32   callHash
 *   33   4    callLen
 *   37   1    enactment.kind               (0=At, 1=After)
 *   38   4    enactment.block
 *   42   1    track                        (0=Root, 1=WhitelistedCaller)
 *   43   20   creator
 *   63   1    N = approvers.length          (≤ 255)
 *   ..  20·N  approvers
 *    .   1    minApprovers                  (≤ N)
 *    .   1    M = approvedBy.length         (≤ N)
 *    .  20·M  approvedBy
 *    .   1    status                        (0/1/2)
 */

export const VERSION = 0x02;
// version + callHash + callLen + enactment(kind+block) + track + creator + N + min.
export const MIN_IDENTITY_LEN = 1 + 32 + 4 + (1 + 4) + 1 + 20 + 1 + 1;
export const MIN_ENCODED_LEN = MIN_IDENTITY_LEN + 1 + 1;
export const MAX_ENCODED_LEN = 416;
/**
 * Hard cap on `approvers.length` for any proposal accepted by the contract.
 * Mirrors `MAX_APPROVERS` in `contract/src/codec.rs`. The codec tests on both
 * sides assert that this many approvers — with all of them having voted — still
 * fits inside {@link MAX_ENCODED_LEN}; if the layout grows past that, those
 * tests fail.
 */
export const MAX_APPROVERS = 8;

export class ProposalCodecError extends Error {
  constructor(public readonly kind: string, message?: string) {
    super(message ?? kind);
    this.name = 'ProposalCodecError';
  }
}

/**
 * Encode the identity prefix of `p` — every field that determines its
 * `proposalKey`. Same wire bytes as the leading section of {@link encodeProposal},
 * so `encodeIdentity(p)` is always a strict prefix of `encodeProposal(p)`.
 *
 * `approvedBy` and `status` are excluded: they're mutable state, not identity,
 * and the key must stay stable across approval and lifecycle transitions.
 */
export function encodeIdentity(p: Proposal): Uint8Array {
  const { n, min } = validateIdentity(p);
  const out = new Uint8Array(MIN_IDENTITY_LEN + 20 * n);
  writeIdentity(out, 0, p, n, min);
  return out;
}

export function encodeProposal(p: Proposal): Uint8Array {
  const { n, min } = validateIdentity(p);
  const m = p.approvedBy.length;
  if (m > 255) throw new ProposalCodecError('LenOverflow');

  const size = MIN_ENCODED_LEN + 20 * (n + m);
  if (size > MAX_ENCODED_LEN) throw new ProposalCodecError('TooLarge');

  const out = new Uint8Array(size);
  let off = writeIdentity(out, 0, p, n, min);

  out[off++] = m;
  for (const a of p.approvedBy) {
    out.set(hexToBytes(a), off); off += 20;
  }

  out[off++] = encodeStatus(p.status);

  if (off !== size) throw new Error('codec bug: size mismatch'); // unreachable
  return out;
}

function validateIdentity(p: Proposal): { n: number; min: number } {
  const n = p.approvers.length;
  if (n > 255) throw new ProposalCodecError('LenOverflow');

  const min = Number(p.minApprovers);
  if (!Number.isInteger(min) || min < 0 || min > 255) {
    throw new ProposalCodecError('LenOverflow', 'minApprovers does not fit in u8');
  }
  if (min > n) throw new ProposalCodecError('MinApproversTooHigh');

  return { n, min };
}

/** Append the identity prefix to `out` at `off`, returning the new offset. */
function writeIdentity(
  out: Uint8Array,
  off: number,
  p: Proposal,
  n: number,
  min: number,
): number {
  out[off++] = VERSION;
  out.set(hexToBytes(p.callHash), off); off += 32;
  writeU32LE(out, off, p.callLen); off += 4;
  out[off++] = encodeDispatchKind(p.enactment.kind);
  writeU32LE(out, off, p.enactment.block); off += 4;
  out[off++] = encodeTrack(p.track);
  out.set(hexToBytes(p.creator), off); off += 20;

  out[off++] = n;
  for (const a of p.approvers) {
    out.set(hexToBytes(a), off); off += 20;
  }

  out[off++] = min;
  return off;
}

export function decodeProposal(bytes: Uint8Array): Proposal {
  const c = new Cursor(bytes);

  const ver = c.readU8();
  if (ver !== VERSION) {
    throw new ProposalCodecError('UnknownVersion', `expected ${VERSION}, got ${ver}`);
  }
  const callHash = c.readHex(32);
  const callLen = c.readU32LE();
  const enactment = { kind: decodeDispatchKind(c.readU8()), block: c.readU32LE() };
  const track = decodeTrack(c.readU8());
  const creator = c.readHex(20);

  const n = c.readU8();
  const approvers: `0x${string}`[] = [];
  for (let i = 0; i < n; i++) approvers.push(c.readHex(20));

  const min = c.readU8();
  if (min > n) throw new ProposalCodecError('MinApproversTooHigh');

  const m = c.readU8();
  const approvedBy: `0x${string}`[] = [];
  for (let i = 0; i < m; i++) approvedBy.push(c.readHex(20));

  const status = decodeStatus(c.readU8());

  if (!c.atEnd) throw new ProposalCodecError('Trailing');

  return {
    callHash,
    callLen,
    enactment,
    track,
    creator,
    approvers,
    minApprovers: BigInt(min),
    approvedBy,
    status,
  };
}

function encodeStatus(s: ProposalStatus): number {
  switch (s) {
    case ProposalStatus.Review: return 0;
    case ProposalStatus.Submitted: return 1;
    case ProposalStatus.Closed: return 2;
    default: throw new ProposalCodecError('BadStatus', `unknown status ${s}`);
  }
}

function decodeStatus(b: number): ProposalStatus {
  switch (b) {
    case 0: return ProposalStatus.Review;
    case 1: return ProposalStatus.Submitted;
    case 2: return ProposalStatus.Closed;
    default: throw new ProposalCodecError('BadStatus', `unknown status ${b}`);
  }
}

function encodeDispatchKind(k: DispatchTimeKind): number {
  switch (k) {
    case DispatchTimeKind.At: return 0;
    case DispatchTimeKind.After: return 1;
    default: throw new ProposalCodecError('BadDispatchKind', `unknown kind ${k}`);
  }
}

function decodeDispatchKind(b: number): DispatchTimeKind {
  switch (b) {
    case 0: return DispatchTimeKind.At;
    case 1: return DispatchTimeKind.After;
    default: throw new ProposalCodecError('BadDispatchKind', `unknown kind ${b}`);
  }
}

function encodeTrack(t: Track): number {
  switch (t) {
    case Track.Root: return 0;
    case Track.WhitelistedCaller: return 1;
    default: throw new ProposalCodecError('BadTrack', `unknown track ${t}`);
  }
}

function decodeTrack(b: number): Track {
  switch (b) {
    case 0: return Track.Root;
    case 1: return Track.WhitelistedCaller;
    default: throw new ProposalCodecError('BadTrack', `unknown track ${b}`);
  }
}

function writeU32LE(out: Uint8Array, off: number, value: number): void {
  out[off]     = value & 0xff;
  out[off + 1] = (value >>> 8) & 0xff;
  out[off + 2] = (value >>> 16) & 0xff;
  out[off + 3] = (value >>> 24) & 0xff;
}

class Cursor {
  constructor(private readonly buf: Uint8Array, private pos = 0) {}

  private take(n: number): Uint8Array {
    if (this.pos + n > this.buf.length) {
      throw new ProposalCodecError('TooShort');
    }
    const out = this.buf.subarray(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }

  readU8(): number {
    return this.take(1)[0];
  }

  readU32LE(): number {
    const s = this.take(4);
    // Cast to unsigned via `>>> 0` so a high bit doesn't accidentally become negative.
    return ((s[0]) | (s[1] << 8) | (s[2] << 16) | (s[3] << 24)) >>> 0;
  }

  readHex(n: number): `0x${string}` {
    return bytesToHex(this.take(n)) as `0x${string}`;
  }

  get atEnd(): boolean {
    return this.pos === this.buf.length;
  }
}

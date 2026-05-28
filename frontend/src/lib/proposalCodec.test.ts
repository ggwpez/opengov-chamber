import { describe, it, expect } from 'vitest';
import { ProposalStatus, type Proposal } from './abi';
import {
  encodeProposal,
  encodeIdentity,
  decodeProposal,
  ProposalCodecError,
  VERSION,
  MIN_ENCODED_LEN,
  MIN_IDENTITY_LEN,
  MAX_ENCODED_LEN,
  MAX_APPROVERS,
} from './proposalCodec';

const repeat = (byte: number, len: number): `0x${string}` =>
  ('0x' + byte.toString(16).padStart(2, '0').repeat(len)) as `0x${string}`;

/**
 * Identical to the `fixture()` defined in `../../../tests/tests/codec.rs`:
 *   callHash = 0xAA·32, callLen = 42, enactmentDelay = 100,
 *   creator = 0x11·20, approvers = [0x22·20, 0x33·20],
 *   minApprovers = 2, approvedBy = [0x22·20], status = Review.
 * Keep both in lockstep.
 */
function fixture(): Proposal {
  return {
    callHash: repeat(0xaa, 32),
    callLen: 42,
    enactmentDelay: 100,
    creator: repeat(0x11, 20),
    approvers: [repeat(0x22, 20), repeat(0x33, 20)],
    minApprovers: 2n,
    approvedBy: [repeat(0x22, 20)],
    status: ProposalStatus.Review,
  };
}

/** Byte-for-byte match with `golden_vector` in `tests/tests/codec.rs`. */
const GOLDEN_HEX =
  // version
  '01' +
  // callHash
  'aa'.repeat(32) +
  // callLen = 42 LE
  '2a000000' +
  // enactmentDelay = 100 LE
  '64000000' +
  // creator
  '11'.repeat(20) +
  // approvers.len = 2
  '02' +
  // approvers
  '22'.repeat(20) +
  '33'.repeat(20) +
  // minApprovers = 2
  '02' +
  // approvedBy.len = 1
  '01' +
  // approvedBy[0]
  '22'.repeat(20) +
  // status = Review
  '00';

const GOLDEN_BYTES = hexToBytes(GOLDEN_HEX);

function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

describe('proposalCodec', () => {
  // THE parity guard. The same bytes are asserted by the Rust `golden_vector`
  // test (tests/tests/codec.rs). If either side drifts, persisted proposals
  // become unreadable — bump VERSION and migrate intentionally.
  it('encodes the canonical proposal to the on-chain golden bytes', () => {
    expect(encodeProposal(fixture())).toEqual(GOLDEN_BYTES);
  });

  it('round-trips the canonical proposal', () => {
    const bytes = encodeProposal(fixture());
    expect(decodeProposal(bytes)).toEqual(fixture());
  });

  it('round-trips an empty-arrays proposal at MIN_ENCODED_LEN', () => {
    const prop: Proposal = {
      callHash: repeat(0, 32),
      callLen: 0,
      enactmentDelay: 0,
      creator: repeat(0, 20),
      approvers: [],
      minApprovers: 0n,
      approvedBy: [],
      status: ProposalStatus.Review,
    };
    const bytes = encodeProposal(prop);
    expect(bytes.length).toBe(MIN_ENCODED_LEN);
    expect(decodeProposal(bytes)).toEqual(prop);
  });

  it('round-trips each status', () => {
    for (const status of [ProposalStatus.Review, ProposalStatus.Submitted, ProposalStatus.Closed]) {
      const prop = { ...fixture(), status };
      expect(decodeProposal(encodeProposal(prop))).toEqual(prop);
    }
  });

  it('fits 17 approvers within the storage cap', () => {
    const approvers = Array.from({ length: 17 }, (_, i) => repeat(i + 1, 20));
    const prop: Proposal = {
      callHash: repeat(0, 32),
      callLen: 0,
      enactmentDelay: 0,
      creator: repeat(0xff, 20),
      approvers,
      minApprovers: 17n,
      approvedBy: [],
      status: ProposalStatus.Review,
    };
    const bytes = encodeProposal(prop);
    expect(bytes.length).toBeLessThanOrEqual(MAX_ENCODED_LEN);
    expect(bytes.length).toBe(65 + 20 * 17);
  });

  it('refuses an 18-approver proposal that would exceed the storage cap', () => {
    const approvers = Array.from({ length: 18 }, (_, i) => repeat(i + 1, 20));
    const prop: Proposal = {
      callHash: repeat(0, 32),
      callLen: 0,
      enactmentDelay: 0,
      creator: repeat(0xff, 20),
      approvers,
      minApprovers: 1n,
      approvedBy: [],
      status: ProposalStatus.Review,
    };
    expect(() => encodeProposal(prop)).toThrow(ProposalCodecError);
  });

  it('refuses minApprovers > approvers.length', () => {
    const prop = { ...fixture(), minApprovers: BigInt(fixture().approvers.length + 1) };
    expect(() => encodeProposal(prop)).toThrow(ProposalCodecError);
  });

  it('refuses minApprovers that does not fit in u8', () => {
    expect(() => encodeProposal({ ...fixture(), minApprovers: 256n })).toThrow(ProposalCodecError);
  });

  it('decode rejects truncated input at every byte', () => {
    const bytes = encodeProposal(fixture());
    for (let cut = 0; cut < bytes.length; cut++) {
      expect(() => decodeProposal(bytes.slice(0, cut)), `cut=${cut}`).toThrow(ProposalCodecError);
    }
  });

  it('decode rejects trailing bytes', () => {
    const bytes = encodeProposal(fixture());
    const extended = new Uint8Array(bytes.length + 1);
    extended.set(bytes);
    expect(() => decodeProposal(extended)).toThrow(ProposalCodecError);
  });

  it('decode rejects an unknown version byte', () => {
    const bytes = encodeProposal(fixture());
    bytes[0] = 0xff;
    expect(() => decodeProposal(bytes)).toThrow(ProposalCodecError);
  });

  it('decode rejects an out-of-range status', () => {
    const bytes = encodeProposal(fixture());
    bytes[bytes.length - 1] = 7;
    expect(() => decodeProposal(bytes)).toThrow(ProposalCodecError);
  });

  it('encodes the documented constants', () => {
    expect(VERSION).toBe(0x01);
    expect(MIN_IDENTITY_LEN).toBe(63);
    expect(MIN_ENCODED_LEN).toBe(65);
    expect(MAX_ENCODED_LEN).toBe(416);
    expect(MAX_APPROVERS).toBe(8);
  });

  // Worst case for the per-proposal lifecycle: every approver has also voted.
  // At `MAX_APPROVERS = 8` the blob is `65 + 20·(8+8) = 385` bytes — under the
  // 416 cap with 31 bytes of headroom. Adding one more would tip it over.
  it('fits MAX_APPROVERS with full approval inside the storage cap', () => {
    const approvers = Array.from({ length: MAX_APPROVERS }, (_, i) => repeat(i + 1, 20));
    const prop: Proposal = {
      callHash: repeat(0xaa, 32),
      callLen: 0xffffffff,
      enactmentDelay: 0xffffffff,
      creator: repeat(0xff, 20),
      approvers,
      minApprovers: BigInt(MAX_APPROVERS),
      approvedBy: approvers, // every approver eventually approves
      status: ProposalStatus.Submitted,
    };
    const bytes = encodeProposal(prop);
    expect(bytes.length).toBe(MIN_ENCODED_LEN + 20 * 2 * MAX_APPROVERS);
    expect(bytes.length).toBeLessThanOrEqual(MAX_ENCODED_LEN);
    expect(decodeProposal(bytes)).toEqual(prop);
  });

  it('refuses MAX_APPROVERS + 1 with full approval', () => {
    const n = MAX_APPROVERS + 1;
    const approvers = Array.from({ length: n }, (_, i) => repeat(i + 1, 20));
    const prop: Proposal = {
      callHash: repeat(0, 32),
      callLen: 0,
      enactmentDelay: 0,
      creator: repeat(0xff, 20),
      approvers,
      minApprovers: BigInt(n),
      approvedBy: approvers,
      status: ProposalStatus.Submitted,
    };
    expect(() => encodeProposal(prop)).toThrow(ProposalCodecError);
  });

  // `proposalKey` derives its hash from `keccak256("Proposal:" || identity)`.
  // If `encodeIdentity` ever stopped being a strict prefix of `encodeProposal`,
  // the on-storage codec and the key derivation would diverge — silently. Mirror
  // of the Rust `encode_identity_is_strict_prefix_of_encode` test.
  it('encodeIdentity is a strict prefix of encodeProposal across mutable state', () => {
    for (const status of [ProposalStatus.Review, ProposalStatus.Submitted, ProposalStatus.Closed]) {
      for (const approvedBy of [[] as `0x${string}`[], [repeat(0x22, 20)]]) {
        const prop: Proposal = { ...fixture(), approvedBy, status };
        const full = encodeProposal(prop);
        const identity = encodeIdentity(prop);
        expect(full.subarray(0, identity.length)).toEqual(identity);
        expect(identity.length).toBe(MIN_IDENTITY_LEN + 20 * prop.approvers.length);
      }
    }
  });
});

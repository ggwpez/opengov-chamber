import { describe, it, expect } from 'vitest';
import { DispatchTimeKind, ProposalStatus, Track, type Proposal } from './abi';
import { proposalKey } from './proposalKey';

/**
 * Mirrors `base()` in `../../../tests/tests/proposal_key.rs`, byte-for-byte, so
 * the two key derivations can be checked against the *same* fixture.
 *   callHash      = 0xAA * 32
 *   callLen       = 42
 *   enactment     = After(100)
 *   track         = WhitelistedCaller
 *   creator       = 0x11 * 20
 *   approvers     = [0x22 * 20, 0x33 * 20]   (strictly ascending)
 *   minApprovers  = 2
 */
const repeat = (byte: number, len: number): `0x${string}` =>
  ('0x' + byte.toString(16).padStart(2, '0').repeat(len)) as `0x${string}`;

function base(): Proposal {
  return {
    callHash: repeat(0xaa, 32),
    callLen: 42,
    enactment: { kind: DispatchTimeKind.After, block: 100 },
    track: Track.WhitelistedCaller,
    creator: repeat(0x11, 20),
    approvers: [repeat(0x22, 20), repeat(0x33, 20)],
    minApprovers: 2n,
    approvedBy: [],
    status: ProposalStatus.Review,
  };
}

describe('proposalKey', () => {
  // THE parity guard. This is the identical vector asserted by the Rust test
  // `proposal_key_golden_hash` (tests/tests/proposal_key.rs). If the TS encoder
  // ever drifts from `contract/src/lib.rs::proposal_key`, approve/finalize would
  // silently target a non-existent key and revert — this test fails first.
  it('matches the on-chain golden hash for the canonical proposal', () => {
    // Asserts the same value as Rust's `proposal_key_golden_hash`
    // (`tests/tests/proposal_key.rs`). The key is now derived as
    // `keccak256(b"Proposal:" || encodeIdentity(base()))`.
    expect(proposalKey(base())).toBe(
      '0x2205874084b7d437a369362692570f4591886f255c23a8510d855e683817ac23',
    );
  });

  it('is deterministic', () => {
    expect(proposalKey(base())).toBe(proposalKey(base()));
  });

  it('ignores approvedBy and status (they are not part of the identity)', () => {
    const key = proposalKey(base());
    expect(proposalKey({ ...base(), approvedBy: [repeat(0x44, 20)] })).toBe(key);
    expect(proposalKey({ ...base(), status: ProposalStatus.Submitted })).toBe(key);
    expect(proposalKey({ ...base(), status: ProposalStatus.Closed })).toBe(key);
  });

  it('changes when any identity field changes', () => {
    const key = proposalKey(base());
    expect(proposalKey({ ...base(), callHash: repeat(0xbb, 32) })).not.toBe(key);
    expect(proposalKey({ ...base(), creator: repeat(0x99, 20) })).not.toBe(key);
    expect(proposalKey({ ...base(), callLen: 43 })).not.toBe(key);
    expect(proposalKey({ ...base(), enactment: { kind: DispatchTimeKind.After, block: 101 } })).not.toBe(key);
    expect(proposalKey({ ...base(), enactment: { kind: DispatchTimeKind.At, block: 100 } })).not.toBe(key);
    expect(proposalKey({ ...base(), track: Track.Root })).not.toBe(key);
    expect(proposalKey({ ...base(), minApprovers: 1n })).not.toBe(key);
    expect(proposalKey({ ...base(), approvers: [repeat(0x22, 20), repeat(0x44, 20)] })).not.toBe(key);
    // Shrinking the approver set also requires shrinking minApprovers — the
    // codec rejects min > N (same guard as the contract's `proposal_key`).
    expect(proposalKey({ ...base(), approvers: [repeat(0x22, 20)], minApprovers: 1n })).not.toBe(key);
  });
});

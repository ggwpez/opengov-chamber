import { keccak256 } from 'viem';
import type { Proposal } from './abi';
import { encodeIdentity } from './proposalCodec';

const DOMAIN = new TextEncoder().encode('Proposal:');

/**
 * Recompute a proposal's storage key, mirroring `proposal_key` in
 * `../../../contract/src/lib.rs` BYTE-FOR-BYTE. `allProposals()` returns the
 * structs but not their keys, yet `approve()` / `finalize()` / `proposal()`
 * are all keyed by this hash — so we derive it client-side.
 *
 * The key is `keccak256(b"Proposal:" || encodeIdentity(prop))`, where
 * {@link encodeIdentity} is the leading section of the on-chain storage
 * codec — every field that gives a proposal its identity, with the same byte
 * layout used by the contract. Sharing the encoder with storage means there's
 * only one place that needs to stay in sync with the Rust side; if the codec's
 * golden vector matches across languages, so does the key.
 */
export function proposalKey(p: Proposal): `0x${string}` {
  const identity = encodeIdentity(p);
  const buf = new Uint8Array(DOMAIN.length + identity.length);
  buf.set(DOMAIN);
  buf.set(identity, DOMAIN.length);
  return keccak256(buf);
}

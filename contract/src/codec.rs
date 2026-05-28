//! Packed storage codec for [`Contract::Proposal`].
//!
//! Solidity-ABI encoding pads every head field to 32 bytes (one EVM word), so
//! a `Proposal` round-trips through `alloy_core::SolValue` at `352 + 32·(N+M)`
//! bytes — colliding with pallet-revive's per-value storage cap of 416 B
//! ([`pallet_revive::limits::STORAGE_BYTES`]) at just N+M = 2. This codec
//! drops the padding for the bytes that actually hit `api::set_storage`,
//! shrinking the same data to `65 + 20·(N+M)` and lifting the cap to N+M ≤ 17.
//!
//! The Solidity ABI is still the wire format on `propose` / `proposal` /
//! `allProposals` / events — only the in-storage blob is packed. Mirror this
//! format in `frontend/src/lib/proposalCodec.ts`.
//!
//! ## Wire format (all little-endian, no padding)
//!
//! ```text
//!  off  size  field
//!    0   1    version (== VERSION)
//!    1   32   callHash
//!   33   4    callLen
//!   37   4    enactmentDelay
//!   41   20   creator
//!   61   1    N = approvers.len()       (≤ 255)
//!   ..  20·N  approvers
//!    .   1    minApprovers              (≤ N, so always fits in u8)
//!    .   1    M = approvedBy.len()      (≤ N)
//!    .  20·M  approvedBy
//!    .   1    status                    (0=Review, 1=Submitted, 2=Closed)
//! ```

use crate::Contract;
use alloc::vec::Vec;
use alloy_core::primitives::{Address, FixedBytes, U256};

/// Bumped whenever the on-storage layout changes incompatibly.
pub const VERSION: u8 = 0x01;

/// Smallest blob the codec can produce: a proposal with no approvers / votes.
pub const MIN_ENCODED_LEN: usize = MIN_IDENTITY_LEN + 1 + 1;
/// Smallest identity prefix: an [`encode_identity`] output with zero approvers.
pub const MIN_IDENTITY_LEN: usize = 1 + 32 + 4 + 4 + 20 + 1 + 1;
/// Pallet-revive caps a single storage value at this many bytes.
pub const MAX_ENCODED_LEN: usize = 416;
/// Hard cap on `approvers.len()` for any proposal accepted by the contract.
///
/// At this size the worst-case storage blob — every approver having also
/// voted, so `M = N` — must still fit inside [`MAX_ENCODED_LEN`]. That
/// invariant is asserted by the `max_approvers_with_full_approval_fits_storage_cap`
/// test; if a future layout change makes 8 no longer fit, the test (rather
/// than a live contract) reports it.
pub const MAX_APPROVERS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    /// The buffer ended before the layout did.
    TooShort,
    /// Bytes remained after the layout finished — the blob does not exactly
    /// match what the parsed lengths describe.
    Trailing,
    /// First byte didn't match [`VERSION`].
    UnknownVersion(u8),
    /// Status byte wasn't 0/1/2.
    BadStatus(u8),
    /// `minApprovers` (a `uint256` at the ABI boundary) didn't fit in a `u8`,
    /// or `approvers.len()` / `approvedBy.len()` did not either.
    LenOverflow,
    /// `minApprovers > approvers.len()` — would always reject every finalize.
    MinApproversTooHigh,
    /// Encoded proposal exceeds [`MAX_ENCODED_LEN`] and could never be stored.
    TooLarge,
}

/// Encode just the identity prefix of `prop` — every field that determines
/// the proposal's [`crate::proposal_key`]. Same wire bytes as [`encode`]'s
/// leading section, so `encode_identity(prop)` is always a strict prefix of
/// `encode(prop)`.
///
/// `approvedBy` and `status` are excluded because they're mutable state, not
/// identity: a proposal that gains an approval or transitions
/// `Review → Submitted` must still hash to the same key.
pub fn encode_identity(prop: &Contract::Proposal) -> Result<Vec<u8>, CodecError> {
    let (n, min) = validate_identity(prop)?;
    let mut out = Vec::with_capacity(MIN_IDENTITY_LEN + 20 * usize::from(n));
    write_identity(&mut out, prop, n, min);
    Ok(out)
}

pub fn encode(prop: &Contract::Proposal) -> Result<Vec<u8>, CodecError> {
    let (n, min) = validate_identity(prop)?;
    let m = u8::try_from(prop.approvedBy.len()).map_err(|_| CodecError::LenOverflow)?;

    let size = MIN_ENCODED_LEN + 20 * (usize::from(n) + usize::from(m));
    if size > MAX_ENCODED_LEN {
        return Err(CodecError::TooLarge);
    }

    let mut out = Vec::with_capacity(size);
    write_identity(&mut out, prop, n, min);
    out.push(m);
    for a in prop.approvedBy.iter() {
        out.extend_from_slice(a.as_slice());
    }
    out.push(encode_status(&prop.status)?);
    debug_assert_eq!(out.len(), size);
    Ok(out)
}

/// Shared validation for the identity-bearing fields. Returns the validated
/// `(approvers_len, minApprovers)` as `u8`s so the writer doesn't recheck.
fn validate_identity(prop: &Contract::Proposal) -> Result<(u8, u8), CodecError> {
    let n = u8::try_from(prop.approvers.len()).map_err(|_| CodecError::LenOverflow)?;
    let min = u256_to_u8(prop.minApprovers).ok_or(CodecError::LenOverflow)?;
    if min > n {
        return Err(CodecError::MinApproversTooHigh);
    }
    Ok((n, min))
}

/// Append the identity prefix to `out`. Caller must have validated via
/// [`validate_identity`] so this can't fail.
fn write_identity(out: &mut Vec<u8>, prop: &Contract::Proposal, n: u8, min: u8) {
    out.push(VERSION);
    out.extend_from_slice(prop.callHash.as_slice());
    out.extend_from_slice(&prop.callLen.to_le_bytes());
    out.extend_from_slice(&prop.enactmentDelay.to_le_bytes());
    out.extend_from_slice(prop.creator.as_slice());
    out.push(n);
    for a in prop.approvers.iter() {
        out.extend_from_slice(a.as_slice());
    }
    out.push(min);
}

pub fn decode(bytes: &[u8]) -> Result<Contract::Proposal, CodecError> {
    let mut c = Cursor::new(bytes);

    let ver = c.read_u8()?;
    if ver != VERSION {
        return Err(CodecError::UnknownVersion(ver));
    }
    let call_hash = c.read_b256()?;
    let call_len = c.read_u32_le()?;
    let enactment_delay = c.read_u32_le()?;
    let creator = c.read_address()?;

    let n = c.read_u8()?;
    let mut approvers = Vec::with_capacity(usize::from(n));
    for _ in 0..n {
        approvers.push(c.read_address()?);
    }

    let min = c.read_u8()?;
    if min > n {
        return Err(CodecError::MinApproversTooHigh);
    }

    let m = c.read_u8()?;
    let mut approved_by = Vec::with_capacity(usize::from(m));
    for _ in 0..m {
        approved_by.push(c.read_address()?);
    }

    let status = decode_status(c.read_u8()?)?;
    if !c.is_empty() {
        return Err(CodecError::Trailing);
    }

    Ok(Contract::Proposal {
        callHash: call_hash,
        callLen: call_len,
        enactmentDelay: enactment_delay,
        creator,
        approvers,
        minApprovers: U256::from(min),
        approvedBy: approved_by,
        status,
    })
}

fn encode_status(s: &Contract::ProposalStatus) -> Result<u8, CodecError> {
    match s {
        Contract::ProposalStatus::Review => Ok(0),
        Contract::ProposalStatus::Submitted => Ok(1),
        Contract::ProposalStatus::Closed => Ok(2),
        // alloy's `sol!` adds a catch-all `__Invalid = u8::MAX` for out-of-range
        // decodes; we never construct one and refuse to persist it.
        other => Err(CodecError::BadStatus(*other as u8)),
    }
}

fn decode_status(b: u8) -> Result<Contract::ProposalStatus, CodecError> {
    Ok(match b {
        0 => Contract::ProposalStatus::Review,
        1 => Contract::ProposalStatus::Submitted,
        2 => Contract::ProposalStatus::Closed,
        x => return Err(CodecError::BadStatus(x)),
    })
}

/// `U256 → u8` losslessly, returning `None` on overflow.
fn u256_to_u8(v: U256) -> Option<u8> {
    let limbs = v.as_limbs();
    if limbs[1] != 0 || limbs[2] != 0 || limbs[3] != 0 {
        return None;
    }
    u8::try_from(limbs[0]).ok()
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.pos.checked_add(n).ok_or(CodecError::TooShort)?;
        if end > self.buf.len() {
            return Err(CodecError::TooShort);
        }
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn read_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32_le(&mut self) -> Result<u32, CodecError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn read_b256(&mut self) -> Result<FixedBytes<32>, CodecError> {
        let s = self.take(32)?;
        let mut a = [0u8; 32];
        a.copy_from_slice(s);
        Ok(FixedBytes(a))
    }

    fn read_address(&mut self) -> Result<Address, CodecError> {
        let s = self.take(20)?;
        let mut a = [0u8; 20];
        a.copy_from_slice(s);
        Ok(Address::from(a))
    }

    fn is_empty(&self) -> bool {
        self.pos == self.buf.len()
    }
}

#![cfg_attr(not(feature = "std"), no_std)]

pub mod codec;
pub mod xcm;

extern crate alloc;

use alloc::vec::Vec;
use alloy_core::{primitives::keccak256, sol};

pub use alloy_core::primitives::{Address, B256, U256};
pub use alloy_core::sol_types;

/// Domain-separation tag prefixed to a proposal's identity bytes before
/// hashing into its storage key.
const PROPOSAL_KEY_DOMAIN: &[u8] = b"Proposal:";

#[cfg(feature = "std")]
sol! {
    #![sol(extra_derives(Debug, PartialEq, Eq))]
    "Contract.sol"
}

#[cfg(not(feature = "std"))]
sol! {
    // `PartialEq`/`Eq` so the contract can match on `ProposalStatus` for its
    // lifecycle guards (`mark_submitted`/`mark_closed`).
    #![sol(extra_derives(PartialEq, Eq))]
    "Contract.sol"
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProposalError {
    ApproversNotStrictlySorted,
    MinApproversTooHigh,
    CreatorIsApprover,
    AlreadyApproved,
    ProposalNotFound,
    NotAnApprover,
    NotApproved,
    NotOwner,
    /// `approvers.len()` exceeds the codec's per-proposal limit (u8).
    TooManyApprovers,
    /// The proposal is no longer in `Review`, so the requested lifecycle
    /// transition (finalize or close) is not allowed.
    ProposalNotInReview,
}

/// Apply the `Review -> Submitted` transition performed by `finalize`.
///
/// Only a proposal still in `Review` can be submitted; submitting an already
/// submitted or closed proposal fails.
pub fn mark_submitted(prop: &mut Contract::Proposal) -> Result<(), ProposalError> {
    expect_review(prop)?;
    prop.status = Contract::ProposalStatus::Submitted;
    Ok(())
}

/// Apply the `Review -> Closed` transition performed by `close`.
///
/// Closing is only possible before finalizing: a `Submitted` proposal cannot be
/// closed, and a `Closed` proposal cannot be closed again.
pub fn mark_closed(prop: &mut Contract::Proposal) -> Result<(), ProposalError> {
    expect_review(prop)?;
    prop.status = Contract::ProposalStatus::Closed;
    Ok(())
}

/// Guard that a proposal is still in `Review`. Used by the lifecycle transitions
/// and by `approve` (approvals are only meaningful before a proposal leaves
/// `Review`).
pub fn expect_review(prop: &Contract::Proposal) -> Result<(), ProposalError> {
    if prop.status == Contract::ProposalStatus::Review {
        Ok(())
    } else {
        Err(ProposalError::ProposalNotInReview)
    }
}

pub fn proposal_key(prop: &Contract::Proposal) -> Result<[u8; 32], ProposalError> {
    // Identity guards: these reject inputs that the codec would also refuse,
    // but with proposal-specific errors so the caller can map them to a
    // meaningful revert reason.
    if prop.approvers.windows(2).any(|w| w[0].0.0 >= w[1].0.0) {
        return Err(ProposalError::ApproversNotStrictlySorted);
    }
    if prop.approvers.len() > u8::MAX as usize {
        return Err(ProposalError::TooManyApprovers);
    }
    if prop.minApprovers > U256::from(prop.approvers.len() as u64) {
        return Err(ProposalError::MinApproversTooHigh);
    }
    if prop.approvers.iter().any(|a| a.0.0 == prop.creator.0.0) {
        return Err(ProposalError::CreatorIsApprover);
    }

    // The codec's identity prefix already covers (in order) callHash, callLen,
    // enactmentDelay, creator, approvers, minApprovers — every field that
    // makes a proposal "the same proposal" — plus the version byte so a
    // future on-storage format bump partitions key space cleanly.
    // The guards above mirror the codec's, so this can't fail in practice.
    let identity = codec::encode_identity(prop).map_err(|e| match e {
        codec::CodecError::LenOverflow => ProposalError::TooManyApprovers,
        codec::CodecError::MinApproversTooHigh => ProposalError::MinApproversTooHigh,
        // Other variants can't be produced by encode_identity.
        _ => ProposalError::TooManyApprovers,
    })?;

    let mut enc = Vec::with_capacity(PROPOSAL_KEY_DOMAIN.len() + identity.len());
    enc.extend_from_slice(PROPOSAL_KEY_DOMAIN);
    enc.extend_from_slice(&identity);
    Ok(keccak256(&enc).0)
}

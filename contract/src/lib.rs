#![cfg_attr(not(feature = "std"), no_std)]

pub mod xcm;

extern crate alloc;

use alloc::vec::Vec;
use alloy_core::{primitives::keccak256, sol};

pub use alloy_core::primitives::{Address, B256, U256};
pub use alloy_core::sol_types;

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

const MAX_PROPOSAL_BYTES: usize = 1024;

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
    if prop.approvers.windows(2).any(|w| w[0].0.0 >= w[1].0.0) {
        return Err(ProposalError::ApproversNotStrictlySorted);
    }
    if prop.minApprovers > U256::from(prop.approvers.len() as u64) {
        return Err(ProposalError::MinApproversTooHigh);
    }
    if prop.approvers.iter().any(|a| a.0.0 == prop.creator.0.0) {
        return Err(ProposalError::CreatorIsApprover);
    }

    let mut enc = Vec::with_capacity(MAX_PROPOSAL_BYTES);
    enc.extend_from_slice(b"Proposal:");
    parity_scale_codec::Encode::encode_to(
        &(prop.creator.0.0, prop.callHash.0),
        &mut enc,
    );
    let len = parity_scale_codec::Compact::<u32>(prop.approvers.len() as u32);
    parity_scale_codec::Encode::encode_to(&len, &mut enc);

    for approver in prop.approvers.iter() {
        parity_scale_codec::Encode::encode_to(&approver.0.0, &mut enc);
    }
    parity_scale_codec::Encode::encode_to(&prop.minApprovers.as_le_bytes(), &mut enc);
    parity_scale_codec::Encode::encode_to(&(prop.callLen, prop.enactmentDelay), &mut enc);

    Ok(keccak256(&enc).0)
}

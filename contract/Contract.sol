// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface Contract {
    // Lifecycle of a proposal.
    //   Review:    just proposed, still collecting approvals; can be finalized or closed.
    //   Submitted: `finalize` was called and the referendum was dispatched. Terminal.
    //   Closed:    `close` was called before finalizing. Terminal; cannot be finalized.
    enum ProposalStatus {
        Review,
        Submitted,
        Closed
    }

    // When an enacted referendum's call runs, mirroring Substrate's
    // `DispatchTime`. Variant indices match its SCALE encoding:
    //   At:    at absolute block number `block`.
    //   After: `block` blocks after the referendum is confirmed.
    enum DispatchTimeKind {
        At,
        After
    }

    struct DispatchTime {
        DispatchTimeKind kind;
        // Absolute target block for `At`; number of blocks to wait for `After`.
        uint32 block;
    }

    // Governance track the contract submits the referendum to.
    //   Root:              the Root origin track.
    //   WhitelistedCaller: the whitelisted-call track (pallet_custom_origins).
    enum Track {
        Root,
        WhitelistedCaller
    }

    struct Proposal {
        // Entry into the preimages pallet
        bytes32 callHash;
        uint32 callLen;

        DispatchTime enactment;
        Track track;

        address creator;

        address[] approvers;
        uint256 minApprovers;
        address[] approvedBy;

        ProposalStatus status;
    }

    event Proposed(bytes32 indexed callHash, address indexed creator, address[] indexed approvers, uint256 minApprovers);
    event Approved(bytes32 indexed proposalHash);
    event Finalized(bytes32 indexed proposalHash, bytes32 indexed callHash);
    event Closed(bytes32 indexed proposalHash);
    event Refunded(address indexed to, uint256 amount);

    error NotApproved();
    error InsufficientDeposit();
    error ProposalNotFound();
    error NotOwner();
    // `destroy` was called while the contract still owes deposits to someone.
    error OutstandingDeposits();

    function allProposals() external view returns (Proposal[] memory);
    function proposal(bytes32 proposalHash) external view returns (Proposal memory);
    // Total funds the contract still owes `depositor`, accumulated across their `finalize` calls.
    function deposits(address depositor) external view returns (uint256);
    // The original deployer ("owner") — the only account `destroy` accepts. Recorded
    // in immutable data at construction.
    function deployer() external view returns (address);

    function propose(bytes32 callHash, uint32 callLen, DispatchTime memory enactment, Track track, address[] memory approvers, uint256 minApprovers) external;
    function approve(bytes32 proposalHash) external;
    function finalize(bytes32 proposalHash) external payable;
    function close(bytes32 proposalHash) external;
    // Refund the caller's entire recorded deposit back to the caller.
    function refund() external;
    // Destroy the contract and send its remaining balance to the original deployer.
    // Reverts unless the caller is the deployer and the contract owes no deposits.
    function destroy() external;
}

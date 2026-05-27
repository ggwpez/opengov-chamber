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

    struct Proposal {
        // Entry into the preimages pallet
        bytes32 callHash;
        uint32 callLen;

        uint32 enactmentDelay;

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

    function allProposals() external view returns (Proposal[] memory);
    function proposal(bytes32 proposalHash) external view returns (Proposal memory);
    // Total funds the contract still owes `depositor`, accumulated across their `finalize` calls.
    function deposits(address depositor) external view returns (uint256);

    function propose(bytes32 callHash, uint32 callLen, uint32 enactmentDelay, address[] memory approvers, uint256 minApprovers) external;
    function approve(bytes32 proposalHash) external;
    function finalize(bytes32 proposalHash) external payable;
    function close(bytes32 proposalHash) external;
    // Refund the caller's entire recorded deposit back to the caller.
    function refund() external;
}

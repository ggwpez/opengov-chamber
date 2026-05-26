// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface Contract {
    struct Proposal {
        // Entry into the preimages pallet
        bytes32 callHash;
        // Encoded byte length of the preimage `callHash` refers to. Required by
        // `Bounded::Lookup { hash, len }` when submitting the referendum.
        uint32 callLen;
        // Blocks to wait after the referendum passes before the call enacts
        // (`DispatchTime::After(enactmentDelay)`).
        uint32 enactmentDelay;
        address creator;
        address[] approvers;
        uint256 minApprovers;
        address[] approvedBy;
    }

    event Proposed(bytes32 indexed callHash, address indexed creator, address[] indexed approvers, uint256 minApprovers);
    event Approved(bytes32 indexed proposalHash);
    event Finalized(bytes32 indexed proposalHash, bytes32 indexed callHash);

    error NotApproved();

    function allProposals() external view returns (Proposal[] memory);
    function proposal(bytes32 proposalHash) external view returns (Proposal memory);

    function propose(bytes32 callHash, uint32 callLen, uint32 enactmentDelay, address[] memory approvers, uint256 minApprovers) external;
    function approve(bytes32 proposalHash) external;
    function finalize(bytes32 proposalHash) external;
}

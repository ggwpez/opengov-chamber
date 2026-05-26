// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface Contract {
    struct Proposal {
        // Entry into the preimages pallet
        bytes32 callHash;
        address creator;
        address[] approvers;
        uint256 minApprovers;
    }

    event Proposed(bytes32 indexed callHash, address indexed creator, address[] indexed approvers, uint256 minApprovers);
    event Approved(bytes32 indexed proposalHash);
    
    error NotApproved();

    function allProposals() external view returns (Proposal[] memory);
    function proposal(bytes32 proposalHash) external view returns (Proposal memory);

    function propose(bytes32 callHash, address[] memory approvers, uint256 minApprovers) external;
    function approve(bytes32 proposalHash) external;
    function finalize(bytes32 proposalHash) external;
}

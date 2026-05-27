/**
 * ABI for `Contract.sol`. Hand-authored from the interface so the frontend has
 * no solc dependency — keep in sync with `../../../contract/Contract.sol`.
 *
 * The Rust contract (compiled to PolkaVM) implements this exact ABI surface;
 * viem encodes/decodes against it the same as for any EVM contract.
 */
export const contractAbi = [
  {
    type: 'function',
    name: 'allProposals',
    stateMutability: 'view',
    inputs: [],
    outputs: [
      {
        name: '',
        type: 'tuple[]',
        components: [
          { name: 'callHash', type: 'bytes32' },
          { name: 'callLen', type: 'uint32' },
          { name: 'enactmentDelay', type: 'uint32' },
          { name: 'creator', type: 'address' },
          { name: 'approvers', type: 'address[]' },
          { name: 'minApprovers', type: 'uint256' },
          { name: 'approvedBy', type: 'address[]' },
        ],
      },
    ],
  },
  {
    type: 'function',
    name: 'proposal',
    stateMutability: 'view',
    inputs: [{ name: 'proposalHash', type: 'bytes32' }],
    outputs: [
      {
        name: '',
        type: 'tuple',
        components: [
          { name: 'callHash', type: 'bytes32' },
          { name: 'callLen', type: 'uint32' },
          { name: 'enactmentDelay', type: 'uint32' },
          { name: 'creator', type: 'address' },
          { name: 'approvers', type: 'address[]' },
          { name: 'minApprovers', type: 'uint256' },
          { name: 'approvedBy', type: 'address[]' },
        ],
      },
    ],
  },
  {
    type: 'function',
    name: 'propose',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'callHash', type: 'bytes32' },
      { name: 'callLen', type: 'uint32' },
      { name: 'enactmentDelay', type: 'uint32' },
      { name: 'approvers', type: 'address[]' },
      { name: 'minApprovers', type: 'uint256' },
    ],
    outputs: [],
  },
  {
    type: 'function',
    name: 'approve',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'proposalHash', type: 'bytes32' }],
    outputs: [],
  },
  {
    type: 'function',
    name: 'finalize',
    stateMutability: 'payable',
    inputs: [{ name: 'proposalHash', type: 'bytes32' }],
    outputs: [],
  },
  {
    type: 'event',
    name: 'Proposed',
    inputs: [
      { name: 'callHash', type: 'bytes32', indexed: true },
      { name: 'creator', type: 'address', indexed: true },
      { name: 'approvers', type: 'address[]', indexed: true },
      { name: 'minApprovers', type: 'uint256', indexed: false },
    ],
    anonymous: false,
  },
  {
    type: 'event',
    name: 'Approved',
    inputs: [{ name: 'proposalHash', type: 'bytes32', indexed: true }],
    anonymous: false,
  },
  {
    type: 'event',
    name: 'Finalized',
    inputs: [
      { name: 'proposalHash', type: 'bytes32', indexed: true },
      { name: 'callHash', type: 'bytes32', indexed: true },
    ],
    anonymous: false,
  },
  { type: 'error', name: 'NotApproved', inputs: [] },
  { type: 'error', name: 'InsufficientDeposit', inputs: [] },
] as const;

/** Shape of a single proposal as returned by `allProposals` / `proposal`. */
export type Proposal = {
  callHash: `0x${string}`;
  callLen: number;
  enactmentDelay: number;
  creator: `0x${string}`;
  approvers: readonly `0x${string}`[];
  minApprovers: bigint;
  approvedBy: readonly `0x${string}`[];
};

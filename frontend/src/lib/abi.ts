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
          {
            name: 'enactment',
            type: 'tuple',
            components: [
              { name: 'kind', type: 'uint8' },
              { name: 'block', type: 'uint32' },
            ],
          },
          { name: 'track', type: 'uint8' },
          { name: 'creator', type: 'address' },
          { name: 'approvers', type: 'address[]' },
          { name: 'minApprovers', type: 'uint256' },
          { name: 'approvedBy', type: 'address[]' },
          { name: 'status', type: 'uint8' },
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
          {
            name: 'enactment',
            type: 'tuple',
            components: [
              { name: 'kind', type: 'uint8' },
              { name: 'block', type: 'uint32' },
            ],
          },
          { name: 'track', type: 'uint8' },
          { name: 'creator', type: 'address' },
          { name: 'approvers', type: 'address[]' },
          { name: 'minApprovers', type: 'uint256' },
          { name: 'approvedBy', type: 'address[]' },
          { name: 'status', type: 'uint8' },
        ],
      },
    ],
  },
  {
    type: 'function',
    name: 'deposits',
    stateMutability: 'view',
    inputs: [{ name: 'depositor', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'propose',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'callHash', type: 'bytes32' },
      { name: 'callLen', type: 'uint32' },
      {
        name: 'enactment',
        type: 'tuple',
        components: [
          { name: 'kind', type: 'uint8' },
          { name: 'block', type: 'uint32' },
        ],
      },
      { name: 'track', type: 'uint8' },
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
    type: 'function',
    name: 'close',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'proposalHash', type: 'bytes32' }],
    outputs: [],
  },
  {
    type: 'function',
    name: 'refund',
    stateMutability: 'nonpayable',
    inputs: [],
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
  {
    type: 'event',
    name: 'Closed',
    inputs: [{ name: 'proposalHash', type: 'bytes32', indexed: true }],
    anonymous: false,
  },
  {
    type: 'event',
    name: 'Refunded',
    inputs: [
      { name: 'to', type: 'address', indexed: true },
      { name: 'amount', type: 'uint256', indexed: false },
    ],
    anonymous: false,
  },
  { type: 'error', name: 'NotApproved', inputs: [] },
  { type: 'error', name: 'InsufficientDeposit', inputs: [] },
  { type: 'error', name: 'ProposalNotFound', inputs: [] },
  { type: 'error', name: 'NotOwner', inputs: [] },
] as const;

/**
 * Lifecycle status of a proposal, ABI-encoded as `uint8`. Mirrors the
 * `ProposalStatus` enum in `Contract.sol`.
 */
export enum ProposalStatus {
  Review = 0,
  Submitted = 1,
  Closed = 2,
}

/**
 * When an enacted referendum's call runs, mirroring Substrate's `DispatchTime`.
 * ABI-encoded as `uint8`; variant indices match its SCALE encoding.
 */
export enum DispatchTimeKind {
  At = 0,
  After = 1,
}

/** Governance track the referendum is submitted to. ABI-encoded as `uint8`. */
export enum Track {
  Root = 0,
  WhitelistedCaller = 1,
}

/**
 * Enactment moment: absolute target block for `At`, or number of blocks to wait
 * for `After`. Mirrors the `DispatchTime` struct in `Contract.sol`.
 */
export type DispatchTime = {
  kind: DispatchTimeKind;
  block: number;
};

/** Shape of a single proposal as returned by `allProposals` / `proposal`. */
export type Proposal = {
  callHash: `0x${string}`;
  callLen: number;
  enactment: DispatchTime;
  track: Track;
  creator: `0x${string}`;
  approvers: readonly `0x${string}`[];
  minApprovers: bigint;
  approvedBy: readonly `0x${string}`[];
  status: ProposalStatus;
};

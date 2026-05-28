import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ProposalCard } from './ProposalCard';
import { DispatchTimeKind, ProposalStatus, Track, type Proposal } from '@/lib/abi';
import { proposalKey } from '@/lib/proposalKey';
import { FINALIZE_DEPOSIT } from '@/lib/constants';

// wagmi + the active-account context are mocked so the card renders with no real
// wallet/chain; we drive the gating inputs (who's connected, the proposal state)
// and assert which action buttons appear/enable and how `writeContract` is called.
const useAccount = vi.fn();
const useWriteContract = vi.fn();
const useWaitForTransactionReceipt = vi.fn();
const useActiveAccount = vi.fn();
const writeContract = vi.fn();

vi.mock('wagmi', () => ({
  useAccount: () => useAccount(),
  useWriteContract: () => useWriteContract(),
  useWaitForTransactionReceipt: () => useWaitForTransactionReceipt(),
}));
vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));
vi.mock('@/lib/activeAccount', () => ({
  useActiveAccount: () => useActiveAccount(),
}));

const CREATOR = '0x1111111111111111111111111111111111111111' as const;
const APPROVER_A = '0x2222222222222222222222222222222222222222' as const;
const APPROVER_B = '0x3333333333333333333333333333333333333333' as const;
const OUTSIDER = '0x9999999999999999999999999999999999999999' as const;

function proposal(overrides: Partial<Proposal> = {}): Proposal {
  return {
    callHash: ('0x' + 'aa'.repeat(32)) as `0x${string}`,
    callLen: 42,
    enactment: { kind: DispatchTimeKind.After, block: 100 },
    track: Track.WhitelistedCaller,
    creator: CREATOR,
    approvers: [APPROVER_A, APPROVER_B],
    minApprovers: 2n,
    approvedBy: [],
    status: ProposalStatus.Review,
    ...overrides,
  };
}

/** Connect (or disconnect) and act as `active`. */
function connectAs(active: `0x${string}` | undefined) {
  useAccount.mockReturnValue({ isConnected: active !== undefined });
  useActiveAccount.mockReturnValue({ activeAddress: active });
}

const btn = (name: RegExp) => screen.getByRole('button', { name });

beforeEach(() => {
  vi.clearAllMocks();
  useWriteContract.mockReturnValue({
    writeContract,
    data: undefined,
    isPending: false,
    error: null,
    reset: vi.fn(),
  });
  useWaitForTransactionReceipt.mockReturnValue({ isLoading: false, isSuccess: false });
});

describe('ProposalCard — approve gating', () => {
  it('enables Approve for a listed approver who has not yet approved', () => {
    connectAs(APPROVER_A);
    render(<ProposalCard proposal={proposal()} index={0} />);
    expect(btn(/approve/i)).toBeEnabled();
  });

  it('disables Approve once that approver has already signed', () => {
    connectAs(APPROVER_A);
    render(<ProposalCard proposal={proposal({ approvedBy: [APPROVER_A] })} index={0} />);
    expect(btn(/approve/i)).toBeDisabled();
  });

  it('disables Approve for an address not on the approver list', () => {
    connectAs(OUTSIDER);
    render(<ProposalCard proposal={proposal()} index={0} />);
    expect(btn(/approve/i)).toBeDisabled();
  });

  it('disables Approve when no wallet is connected', () => {
    connectAs(undefined);
    render(<ProposalCard proposal={proposal()} index={0} />);
    expect(btn(/approve/i)).toBeDisabled();
  });

  it('calls writeContract with functionName "approve" and the recomputed key', () => {
    connectAs(APPROVER_A);
    const p = proposal();
    render(<ProposalCard proposal={p} index={0} />);
    fireEvent.click(btn(/approve/i));
    expect(writeContract).toHaveBeenCalledTimes(1);
    expect(writeContract.mock.calls[0][0]).toMatchObject({
      functionName: 'approve',
      args: [proposalKey(p)],
    });
  });
});

describe('ProposalCard — finalize gating', () => {
  it('hides Finalize/Cancel from non-creators', () => {
    connectAs(APPROVER_A);
    render(<ProposalCard proposal={proposal()} index={0} />);
    expect(screen.queryByRole('button', { name: /finalize/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /cancel/i })).toBeNull();
  });

  it('shows Finalize to the creator but disables it below threshold', () => {
    connectAs(CREATOR);
    render(<ProposalCard proposal={proposal({ approvedBy: [APPROVER_A] })} index={0} />);
    expect(btn(/finalize/i)).toBeDisabled();
  });

  it('enables Finalize once approvals meet minApprovers', () => {
    connectAs(CREATOR);
    render(<ProposalCard proposal={proposal({ approvedBy: [APPROVER_A, APPROVER_B] })} index={0} />);
    expect(btn(/finalize/i)).toBeEnabled();
  });

  it('forwards the submission deposit as value when finalizing', () => {
    connectAs(CREATOR);
    const p = proposal({ approvedBy: [APPROVER_A, APPROVER_B] });
    render(<ProposalCard proposal={p} index={0} />);
    fireEvent.click(btn(/finalize/i));
    expect(writeContract.mock.calls[0][0]).toMatchObject({
      functionName: 'finalize',
      args: [proposalKey(p)],
      value: FINALIZE_DEPOSIT,
    });
  });
});

describe('ProposalCard — cancel gating', () => {
  it('lets the creator cancel a Review proposal via close()', () => {
    connectAs(CREATOR);
    const p = proposal();
    render(<ProposalCard proposal={p} index={0} />);
    fireEvent.click(btn(/cancel/i));
    expect(writeContract.mock.calls[0][0]).toMatchObject({ functionName: 'close', args: [proposalKey(p)] });
  });
});

describe('ProposalCard — terminal proposals show no actions', () => {
  it('renders the "submitted" badge and no buttons when Submitted', () => {
    connectAs(CREATOR);
    render(<ProposalCard proposal={proposal({ status: ProposalStatus.Submitted })} index={0} />);
    expect(screen.getByText('submitted')).toBeInTheDocument();
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('renders the "cancelled" badge and no buttons when Closed', () => {
    connectAs(CREATOR);
    render(<ProposalCard proposal={proposal({ status: ProposalStatus.Closed })} index={0} />);
    expect(screen.getByText('cancelled')).toBeInTheDocument();
    expect(screen.queryByRole('button')).toBeNull();
  });
});

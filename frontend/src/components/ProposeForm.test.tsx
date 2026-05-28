import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ProposeForm } from './ProposeForm';
import { DispatchTimeKind, Track } from '@/lib/abi';
import { paseoHub } from '@/lib/chain';

const useAccount = vi.fn();
const useSwitchChain = vi.fn();
const useWriteContract = vi.fn();
const useWaitForTransactionReceipt = vi.fn();
const useActiveAccount = vi.fn();
const writeContract = vi.fn();
const switchChain = vi.fn();

vi.mock('wagmi', () => ({
  useAccount: () => useAccount(),
  useSwitchChain: () => useSwitchChain(),
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
const APPROVER = '0x2222222222222222222222222222222222222222' as const;
const HASH32 = ('0x' + 'a'.repeat(64)) as `0x${string}`;

const createBtn = () => screen.getByRole('button', { name: /create proposal/i });
const hashInput = () => screen.getByPlaceholderText('0x…');
const lenInput = () => screen.getByPlaceholderText('e.g. 42');
const approversInput = () => screen.getByPlaceholderText(/one address per line/i);

/** Fill the hash-mode form with a valid hash, length and single approver. */
function fillValid() {
  fireEvent.change(hashInput(), { target: { value: HASH32 } });
  fireEvent.change(lenInput(), { target: { value: '42' } });
  fireEvent.change(approversInput(), { target: { value: APPROVER } });
}

beforeEach(() => {
  vi.clearAllMocks();
  useAccount.mockReturnValue({ isConnected: true, chainId: paseoHub.id });
  useActiveAccount.mockReturnValue({ activeAddress: CREATOR });
  useSwitchChain.mockReturnValue({ switchChain, isPending: false });
  useWriteContract.mockReturnValue({
    writeContract,
    data: undefined,
    isPending: false,
    error: null,
    reset: vi.fn(),
  });
  useWaitForTransactionReceipt.mockReturnValue({ isLoading: false, isSuccess: false });
});

describe('ProposeForm — validation gating', () => {
  it('disables submit until the form is valid', () => {
    render(<ProposeForm />);
    expect(createBtn()).toBeDisabled();
  });

  it('enables submit once hash, length and an approver are valid', () => {
    render(<ProposeForm />);
    fillValid();
    expect(createBtn()).toBeEnabled();
  });

  it('rejects the creator listing themselves as an approver', () => {
    render(<ProposeForm />);
    fireEvent.change(hashInput(), { target: { value: HASH32 } });
    fireEvent.change(lenInput(), { target: { value: '42' } });
    fireEvent.change(approversInput(), { target: { value: CREATOR } });
    expect(createBtn()).toBeDisabled();
    expect(screen.getByText(/own address cannot be an approver/i)).toBeInTheDocument();
  });

  it('rejects a min-approvers threshold above the approver count', () => {
    render(<ProposeForm />);
    fillValid();
    // default min is 1 and one approver → bump min to 2.
    fireEvent.change(screen.getByDisplayValue('1'), { target: { value: '2' } });
    expect(createBtn()).toBeDisabled();
    expect(screen.getByText(/min approvers exceeds approver count/i)).toBeInTheDocument();
  });
});

describe('ProposeForm — wrong network', () => {
  it('blocks submit and offers a chain switch when the wallet is on another chain', () => {
    useAccount.mockReturnValue({ isConnected: true, chainId: 1 });
    render(<ProposeForm />);
    fillValid();
    expect(createBtn()).toBeDisabled();
    const switchBtn = screen.getByRole('button', { name: /switch to/i });
    fireEvent.click(switchBtn);
    expect(switchChain).toHaveBeenCalledWith({ chainId: paseoHub.id });
  });
});

describe('ProposeForm — submission', () => {
  it('calls writeContract with propose args pinned to Paseo Hub', () => {
    render(<ProposeForm />);
    fillValid();
    fireEvent.click(createBtn());
    expect(writeContract).toHaveBeenCalledTimes(1);
    expect(writeContract.mock.calls[0][0]).toMatchObject({
      functionName: 'propose',
      chainId: paseoHub.id,
      account: CREATOR,
      args: [
        HASH32,
        42,
        { kind: DispatchTimeKind.After, block: 100 },
        Track.WhitelistedCaller,
        [APPROVER],
        1n,
      ],
    });
  });
});

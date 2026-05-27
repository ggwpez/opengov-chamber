'use client';

import { useState } from 'react';
import { useAccount, useReadContract, useSwitchChain } from 'wagmi';
import { paseoHub } from '@/lib/chain';
import { shorten } from '@/lib/format';
import { contractAbi } from '@/lib/abi';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { useActiveAccount } from '@/lib/activeAccount';
import { ConnectModal } from './ConnectModal';
import { WithdrawModal } from './WithdrawModal';

export function ConnectButton() {
  // `chainId` here is the *wallet's* connected chain (from useAccount), which
  // reflects networks outside our config too. Do NOT use useChainId() — that's
  // config-scoped and can report paseoHub even while the wallet is on mainnet.
  const { isConnected, chainId } = useAccount();
  const { switchChain, isPending: switching } = useSwitchChain();
  const { activeAddress } = useActiveAccount();
  const [open, setOpen] = useState(false);
  const [withdrawOpen, setWithdrawOpen] = useState(false);

  // Show the Withdraw button only when the active account has a balance the
  // contract still owes it (its `deposits` tally), and we're on the right chain.
  const { data: owed } = useReadContract({
    address: CONTRACT_ADDRESS,
    abi: contractAbi,
    functionName: 'deposits',
    args: activeAddress ? [activeAddress] : undefined,
    query: {
      enabled: CONTRACT_CONFIGURED && !!activeAddress && chainId === paseoHub.id,
      refetchInterval: 12_000,
    },
  });
  const canWithdraw = ((owed as bigint | undefined) ?? 0n) > 0n;

  if (!isConnected) {
    return (
      <>
        <button className="btn btn-primary" onClick={() => setOpen(true)}>
          Connect wallet
        </button>
        {open && <ConnectModal onClose={() => setOpen(false)} />}
      </>
    );
  }

  if (chainId !== paseoHub.id) {
    return (
      <button
        className="btn"
        disabled={switching}
        onClick={() => switchChain({ chainId: paseoHub.id })}
      >
        {switching ? 'Switching…' : 'Switch to Paseo Hub'}
      </button>
    );
  }

  return (
    <div className="wallet">
      <button className="addr-pill" title="Switch wallet or account" onClick={() => setOpen(true)}>
        {shorten(activeAddress ?? '')}
      </button>
      {canWithdraw && (
        <button className="btn btn-primary" onClick={() => setWithdrawOpen(true)}>
          Withdraw
        </button>
      )}
      {open && <ConnectModal onClose={() => setOpen(false)} />}
      {withdrawOpen && <WithdrawModal onClose={() => setWithdrawOpen(false)} />}
    </div>
  );
}

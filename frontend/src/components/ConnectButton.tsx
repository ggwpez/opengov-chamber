'use client';

import { useState } from 'react';
import { useAccount, useDisconnect, useSwitchChain } from 'wagmi';
import { paseoHub } from '@/lib/chain';
import { shorten } from '@/lib/format';
import { useActiveAccount } from '@/lib/activeAccount';
import { ConnectModal } from './ConnectModal';

export function ConnectButton() {
  // `chainId` here is the *wallet's* connected chain (from useAccount), which
  // reflects networks outside our config too. Do NOT use useChainId() — that's
  // config-scoped and can report paseoHub even while the wallet is on mainnet.
  const { isConnected, chainId } = useAccount();
  const { disconnect } = useDisconnect();
  const { switchChain, isPending: switching } = useSwitchChain();
  const { activeAddress } = useActiveAccount();
  const [open, setOpen] = useState(false);

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
      <span className="dot" />
      <button className="addr-pill" title="Switch wallet or account" onClick={() => setOpen(true)}>
        {shorten(activeAddress ?? '')}
      </button>
      <button className="btn" onClick={() => disconnect()}>
        Disconnect
      </button>
      {open && <ConnectModal onClose={() => setOpen(false)} />}
    </div>
  );
}

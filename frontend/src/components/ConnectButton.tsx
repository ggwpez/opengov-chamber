'use client';

import { useAccount, useConnect, useDisconnect, useSwitchChain } from 'wagmi';
import { injected } from 'wagmi/connectors';
import { paseoHub } from '@/lib/chain';
import { shorten } from '@/lib/format';

export function ConnectButton() {
  // `chainId` here is the *wallet's* connected chain (from useAccount), which
  // reflects networks outside our config too. Do NOT use useChainId() — that's
  // config-scoped and can report paseoHub even while the wallet is on mainnet.
  const { address, isConnected, chainId } = useAccount();
  const { connect, isPending } = useConnect();
  const { disconnect } = useDisconnect();
  const { switchChain, isPending: switching } = useSwitchChain();

  if (!isConnected) {
    return (
      <button
        className="btn btn-primary"
        disabled={isPending}
        onClick={() => connect({ connector: injected() })}
      >
        {isPending ? 'Connecting…' : 'Connect wallet'}
      </button>
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
      <span className="addr-pill">{shorten(address ?? '')}</span>
      <button className="btn" onClick={() => disconnect()}>
        Disconnect
      </button>
    </div>
  );
}

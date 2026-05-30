'use client';

import { useState } from 'react';
import {
  useAccount,
  useReadContract,
  useWaitForTransactionReceipt,
  useWriteContract,
} from 'wagmi';
import { BaseError } from 'viem';
import { useQueryClient } from '@tanstack/react-query';
import { contractAbi } from '@/lib/abi';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { chain } from '@/lib/chain';
import { useActiveAccount } from '@/lib/activeAccount';

/**
 * Footer action shown only when the owner (the contract's deployer) is the
 * active account: tears the whole contract down via `destroy()`, sweeping its
 * balance back to the deployer. The call reverts unless the contract owes no
 * deposits. It's irreversible, so the button arms on first click and only fires
 * on confirm. The owner is read on-chain from the contract's `deployer()`
 * getter — the same account `destroy()` enforces.
 */
export function DestroyButton() {
  // `chainId` is the wallet's connected chain (see ConnectButton) — gate on it
  // so we never offer a write while the wallet is on the wrong network.
  const { isConnected, chainId } = useAccount();
  const { activeAddress } = useActiveAccount();
  const queryClient = useQueryClient();
  const [armed, setArmed] = useState(false);

  // The deployer is immutable, so read it once (no polling).
  const { data: deployer } = useReadContract({
    address: CONTRACT_ADDRESS,
    abi: contractAbi,
    functionName: 'deployer',
    query: { enabled: CONTRACT_CONFIGURED },
  });

  const { writeContract, data: hash, isPending, error, reset } = useWriteContract();
  const { isLoading: confirming, isSuccess } = useWaitForTransactionReceipt({ hash });

  const isOwner =
    !!activeAddress && !!deployer && activeAddress.toLowerCase() === deployer.toLowerCase();

  if (!CONTRACT_CONFIGURED || !isConnected || chainId !== chain.id || !isOwner) {
    return null;
  }

  const busy = isPending || confirming;

  function destroy() {
    reset();
    writeContract(
      {
        account: activeAddress,
        chainId: chain.id,
        address: CONTRACT_ADDRESS,
        abi: contractAbi,
        functionName: 'destroy',
        args: [],
      },
      { onSuccess: () => queryClient.invalidateQueries() },
    );
  }

  if (isSuccess) {
    return <span className="sitefoot-meta">Contract destroyed</span>;
  }

  return (
    <span className="sitefoot-destroy">
      {error && (
        <span className="sitefoot-destroy-err">
          {(error as BaseError).shortMessage ?? error.message}
        </span>
      )}
      {armed ? (
        <>
          <button className="btn btn-danger btn-xs" disabled={busy} onClick={destroy}>
            {busy ? 'Destroying…' : 'Confirm — irreversible'}
          </button>
          {!busy && (
            <button className="btn btn-xs" onClick={() => setArmed(false)}>
              Cancel
            </button>
          )}
        </>
      ) : (
        <button className="btn btn-danger btn-xs" onClick={() => setArmed(true)}>
          Destroy contract
        </button>
      )}
    </span>
  );
}

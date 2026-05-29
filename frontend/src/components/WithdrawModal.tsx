'use client';

import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useBalance, useReadContract, useWaitForTransactionReceipt, useWriteContract } from 'wagmi';
import { BaseError, formatEther } from 'viem';
import { contractAbi } from '@/lib/abi';
import { CONTRACT_ADDRESS } from '@/lib/contract';
import { CHAIN_SYMBOL } from '@/lib/chain';
import { useActiveAccount } from '@/lib/activeAccount';

/**
 * Shows what the contract still owes the active account (its `deposits` tally)
 * alongside the contract's own balance, and lets the user reclaim the full
 * amount via `refund()`.
 */
export function WithdrawModal({ onClose }: { onClose: () => void }) {
  const { activeAddress } = useActiveAccount();
  const queryClient = useQueryClient();

  const { data: owed } = useReadContract({
    address: CONTRACT_ADDRESS,
    abi: contractAbi,
    functionName: 'deposits',
    args: activeAddress ? [activeAddress] : undefined,
    query: { enabled: !!activeAddress },
  });

  const { data: contractBalance } = useBalance({ address: CONTRACT_ADDRESS });

  const { writeContract, data: hash, isPending, error, reset } = useWriteContract();
  const { isLoading: confirming, isSuccess } = useWaitForTransactionReceipt({ hash });

  // Close on Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const owedAmount = (owed as bigint | undefined) ?? 0n;
  const busy = isPending || confirming;
  const nothingOwed = owedAmount === 0n;

  function withdraw() {
    reset();
    writeContract(
      { account: activeAddress, address: CONTRACT_ADDRESS, abi: contractAbi, functionName: 'refund', args: [] },
      { onSuccess: () => queryClient.invalidateQueries() },
    );
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Withdraw"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <h2>Withdraw</h2>
          <button className="modal-x" aria-label="Close" onClick={onClose}>
            ✕
          </button>
        </div>

        <dl className="kv" style={{ marginTop: 16 }}>
          <div>
            <dt>Owed to you</dt>
            <dd>{formatEther(owedAmount)} {CHAIN_SYMBOL}</dd>
          </div>
          <div>
            <dt>Contract balance</dt>
            <dd>{contractBalance ? `${formatEther(contractBalance.value)} ${CHAIN_SYMBOL}` : '…'}</dd>
          </div>
        </dl>

        {isSuccess ? (
          <div className="notice ok">Withdrawal sent.</div>
        ) : (
          error && <div className="notice err">{(error as BaseError).shortMessage ?? error.message}</div>
        )}

        <div className="modal-foot">
          <button className="btn btn-primary" onClick={withdraw} disabled={busy || nothingOwed || isSuccess}>
            {busy ? 'Withdrawing…' : nothingOwed ? 'Nothing to withdraw' : `Withdraw ${formatEther(owedAmount)} ${CHAIN_SYMBOL}`}
          </button>
        </div>
      </div>
    </div>
  );
}

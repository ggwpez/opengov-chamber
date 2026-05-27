'use client';

import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useAccount, useWaitForTransactionReceipt, useWriteContract } from 'wagmi';
import { BaseError, formatEther } from 'viem';
import { contractAbi, type Proposal } from '@/lib/abi';
import { CONTRACT_ADDRESS } from '@/lib/contract';
import { FINALIZE_DEPOSIT } from '@/lib/constants';
import { proposalKey } from '@/lib/proposalKey';
import { shorten } from '@/lib/format';
import { useActiveAccount } from '@/lib/activeAccount';

export function ProposalCard({ proposal, index }: { proposal: Proposal; index: number }) {
  const { isConnected } = useAccount();
  const { activeAddress: address } = useActiveAccount();
  const queryClient = useQueryClient();
  const [action, setAction] = useState<'approve' | 'finalize' | null>(null);

  const key = proposalKey(proposal);
  const approved = proposal.approvedBy.map((a) => a.toLowerCase());
  const required = Number(proposal.minApprovers);
  const count = proposal.approvedBy.length;
  const ready = count >= required;
  const pct = required === 0 ? 100 : Math.min(100, Math.round((count / required) * 100));

  const isApprover = !!address && proposal.approvers.some((a) => a.toLowerCase() === address.toLowerCase());
  const hasApproved = !!address && approved.includes(address.toLowerCase());
  const canApprove = isConnected && isApprover && !hasApproved;

  const { writeContract, data: hash, isPending, error, reset } = useWriteContract();
  const { isLoading: confirming, isSuccess } = useWaitForTransactionReceipt({ hash });

  function run(which: 'approve' | 'finalize') {
    reset();
    setAction(which);
    const onSettled = { onSuccess: () => queryClient.invalidateQueries() };
    if (which === 'approve') {
      writeContract(
        { account: address, address: CONTRACT_ADDRESS, abi: contractAbi, functionName: 'approve', args: [key] },
        onSettled,
      );
    } else {
      writeContract(
        { account: address, address: CONTRACT_ADDRESS, abi: contractAbi, functionName: 'finalize', args: [key], value: FINALIZE_DEPOSIT },
        onSettled,
      );
    }
  }

  const busy = isPending || confirming;

  return (
    <article className={`card rise ${ready ? 'is-ready' : ''}`} style={{ animationDelay: `${index * 60}ms` }}>
      <div className="card-head">
        <div>
          <div className="card-hash">{shorten(proposal.callHash, 10, 8)}</div>
          <div className="card-sub">key {shorten(key, 10, 8)}</div>
        </div>
        <span className={`tag ${ready ? 'ready' : 'pending'}`}>{ready ? 'ready to submit' : 'collecting'}</span>
      </div>

      <dl className="kv">
        <div>
          <dt>Creator</dt>
          <dd>{shorten(proposal.creator)}</dd>
        </div>
        <div>
          <dt>Call length</dt>
          <dd>{proposal.callLen} bytes</dd>
        </div>
        <div>
          <dt>Enactment delay</dt>
          <dd>{proposal.enactmentDelay} blocks</dd>
        </div>
        <div>
          <dt>Approvals</dt>
          <dd>
            {count} / {required}
            <div className={`meter ${ready ? 'full' : ''}`}>
              <span style={{ width: `${pct}%` }} />
            </div>
          </dd>
        </div>
      </dl>

      <div className="approvers">
        {proposal.approvers.map((a) => (
          <span key={a} className={`chip ${approved.includes(a.toLowerCase()) ? 'signed' : ''}`}>
            {approved.includes(a.toLowerCase()) ? '✓ ' : ''}
            {shorten(a, 5, 4)}
          </span>
        ))}
      </div>

      <div className="row spread" style={{ marginTop: 18 }}>
        <span className="muted mono" style={{ fontSize: 11 }}>
          {hasApproved ? 'you signed' : isApprover ? 'you can sign' : 'observer'}
        </span>
        <div className="row">
          <button className="btn" disabled={!canApprove || busy} onClick={() => run('approve')}>
            {busy && action === 'approve' ? 'Approving…' : 'Approve'}
          </button>
          <button className="btn btn-mint" disabled={!ready || !isConnected || busy} onClick={() => run('finalize')}>
            {busy && action === 'finalize' ? 'Submitting…' : `Finalize · ${formatEther(FINALIZE_DEPOSIT)} PAS`}
          </button>
        </div>
      </div>

      {isSuccess && action === 'approve' && <div className="notice ok">Approval recorded.</div>}
      {isSuccess && action === 'finalize' && (
        <div className="notice ok">Finalized — referendum submission dispatched via the XCM precompile.</div>
      )}
      {error && <div className="notice err">{(error as BaseError).shortMessage ?? error.message}</div>}
    </article>
  );
}

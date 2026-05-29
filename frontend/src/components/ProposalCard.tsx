'use client';

import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useAccount, useWaitForTransactionReceipt, useWriteContract } from 'wagmi';
import { BaseError, formatEther } from 'viem';
import { contractAbi, DispatchTimeKind, ProposalStatus, Track, type Proposal } from '@/lib/abi';
import { CONTRACT_ADDRESS } from '@/lib/contract';
import { FINALIZE_DEPOSIT } from '@/lib/constants';
import { proposalKey } from '@/lib/proposalKey';
import { CHAIN_SYMBOL, explorerAddressUrl, explorerPreimageUrl } from '@/lib/chain';
import { shorten } from '@/lib/format';
import { useActiveAccount } from '@/lib/activeAccount';

export function ProposalCard({ proposal, index }: { proposal: Proposal; index: number }) {
  const { isConnected } = useAccount();
  const { activeAddress: address } = useActiveAccount();
  const queryClient = useQueryClient();
  const [action, setAction] = useState<'approve' | 'finalize' | 'close' | null>(null);

  const key = proposalKey(proposal);
  const approved = proposal.approvedBy.map((a) => a.toLowerCase());
  const required = Number(proposal.minApprovers);
  const count = proposal.approvedBy.length;
  const ready = count >= required;
  const pct = required === 0 ? 100 : Math.min(100, Math.round((count / required) * 100));

  // Only proposals in `Review` accept any on-chain action; `Submitted`/`Closed`
  // are terminal and the contract reverts every write against them.
  const isReview = proposal.status === ProposalStatus.Review;
  const isSubmitted = proposal.status === ProposalStatus.Submitted;
  const isClosed = proposal.status === ProposalStatus.Closed;

  const isApprover = !!address && proposal.approvers.some((a) => a.toLowerCase() === address.toLowerCase());
  const isCreator = !!address && proposal.creator.toLowerCase() === address.toLowerCase();
  const hasApproved = !!address && approved.includes(address.toLowerCase());
  const canApprove = isReview && isConnected && isApprover && !hasApproved;
  const canFinalize = isReview && isConnected && isCreator && ready;
  const canClose = isReview && isConnected && isCreator;

  const { writeContract, data: hash, isPending, error, reset } = useWriteContract();
  const { isLoading: confirming, isSuccess } = useWaitForTransactionReceipt({ hash });

  function run(which: 'approve' | 'finalize' | 'close') {
    reset();
    setAction(which);
    const onSettled = { onSuccess: () => queryClient.invalidateQueries() };
    if (which === 'finalize') {
      writeContract(
        { account: address, address: CONTRACT_ADDRESS, abi: contractAbi, functionName: 'finalize', args: [key], value: FINALIZE_DEPOSIT },
        onSettled,
      );
    } else {
      writeContract(
        { account: address, address: CONTRACT_ADDRESS, abi: contractAbi, functionName: which, args: [key] },
        onSettled,
      );
    }
  }

  const busy = isPending || confirming;

  const statusTag = isClosed
    ? {
        cls: 'closed',
        label: 'cancelled',
        title: 'Closed by the creator before finalizing. Terminal — no further action is possible.',
      }
    : isSubmitted
      ? {
          cls: 'submitted',
          label: 'submitted',
          title:
            'Finalized: the contract dispatched Referenda::submit on-chain via the XCM precompile. Terminal — no further action is possible.',
        }
      : ready
        ? {
            cls: 'ready',
            label: 'ready to submit',
            title: `Threshold met (${count}/${required} approvals). The creator can now finalize to submit the referendum.`,
          }
        : {
            cls: 'pending',
            label: 'under review',
            title: `Still in review, collecting approvals (${count}/${required}). Listed approvers can sign; the creator can cancel.`,
          };

  return (
    <article
      className={`card rise ${isClosed ? 'is-closed' : isSubmitted ? 'is-submitted' : ready ? 'is-ready' : ''}`}
      style={{ animationDelay: `${index * 60}ms` }}
    >
      <div className="card-head">
        <div>
          <div className={`card-hash ${isClosed ? 'struck' : ''}`}>
            Call hash:{' '}
            {(() => {
              const url = explorerPreimageUrl(proposal.callHash);
              return url ? (
                <a href={url} target="_blank" rel="noreferrer" className="addr-link">
                  {proposal.callHash}
                </a>
              ) : (
                proposal.callHash
              );
            })()}
          </div>
        </div>
        <span className={`tag ${statusTag.cls}`} title={statusTag.title}>
          {statusTag.label}
        </span>
      </div>

      <dl className="kv">
        <div>
          <dt>Creator</dt>
          <dd>
            {(() => {
              const url = explorerAddressUrl(proposal.creator);
              return url ? (
                <a href={url} target="_blank" rel="noreferrer" className="addr-plain">
                  {shorten(proposal.creator)}
                </a>
              ) : (
                shorten(proposal.creator)
              );
            })()}
          </dd>
        </div>
        <div>
          <dt>Call length</dt>
          <dd>{proposal.callLen} bytes</dd>
        </div>
        <div>
          <dt>Enactment</dt>
          <dd>
            {proposal.enactment.kind === DispatchTimeKind.At
              ? `at block ${proposal.enactment.block}`
              : `after ${proposal.enactment.block} blocks`}
          </dd>
        </div>
        <div>
          <dt>Track</dt>
          <dd>{proposal.track === Track.Root ? 'Root' : 'Whitelisted caller'}</dd>
        </div>
      </dl>

      <div className="approvers-block">
        <div className="approvers-head">
          <span className="approvers-label">
            {count} of {required} approved
          </span>
        </div>
        <div className={`meter ${ready ? 'full' : ''}`}>
          <span style={{ width: `${pct}%` }} />
        </div>
        <div className="approvers">
          {proposal.approvers.map((a) => {
            const isSigned = approved.includes(a.toLowerCase());
            const label = `${isSigned ? '✓ ' : ''}${shorten(a, 5, 4)}`;
            const url = explorerAddressUrl(a);
            return url ? (
              <a
                key={a}
                href={url}
                target="_blank"
                rel="noreferrer"
                className={`chip ${isSigned ? 'signed' : ''}`}
              >
                {label}
              </a>
            ) : (
              <span key={a} className={`chip ${isSigned ? 'signed' : ''}`}>
                {label}
              </span>
            );
          })}
        </div>
      </div>

      {isReview && (
        <div className="row" style={{ marginTop: 18, justifyContent: 'flex-end' }}>
          <button
            className="btn"
            disabled={!canApprove || busy}
            onClick={() => run('approve')}
            title={
              canApprove || busy
                ? undefined
                : !isConnected
                  ? 'Connect a wallet to approve.'
                  : !isApprover
                    ? 'Only listed approvers can approve this proposal.'
                    : hasApproved
                      ? "You've already approved this proposal."
                      : undefined
            }
          >
            {busy && action === 'approve' ? 'Approving…' : 'Approve'}
          </button>
          {isCreator && (
            <button className="btn btn-mint" disabled={!canFinalize || busy} onClick={() => run('finalize')}>
              {busy && action === 'finalize' ? 'Submitting…' : `Finalize · ${formatEther(FINALIZE_DEPOSIT)} ${CHAIN_SYMBOL}`}
            </button>
          )}
          {canClose && (
            <button className="btn btn-danger" disabled={busy} onClick={() => run('close')}>
              {busy && action === 'close' ? 'Cancelling…' : 'Cancel'}
            </button>
          )}
        </div>
      )}

      {isSuccess && action === 'approve' && <div className="notice ok">Approval recorded.</div>}
      {isSuccess && action === 'finalize' && (
        <div className="notice ok">Finalized — referendum submission dispatched via the XCM precompile.</div>
      )}
      {isSuccess && action === 'close' && <div className="notice ok">Proposal cancelled.</div>}
      {error && <div className="notice err">{(error as BaseError).shortMessage ?? error.message}</div>}
    </article>
  );
}

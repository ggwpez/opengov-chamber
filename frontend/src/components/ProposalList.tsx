'use client';

import { useReadContract } from 'wagmi';
import { contractAbi, type Proposal } from '@/lib/abi';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { ProposalCard } from './ProposalCard';

export function ProposalList() {
  const { data, isLoading, isError, error } = useReadContract({
    address: CONTRACT_ADDRESS,
    abi: contractAbi,
    functionName: 'allProposals',
    query: { enabled: CONTRACT_CONFIGURED, refetchInterval: 12_000 },
  });

  if (!CONTRACT_CONFIGURED) {
    return (
      <div className="empty">
        No contract address configured. Set <span className="mono">NEXT_PUBLIC_CONTRACT_ADDRESS</span> in
        <span className="mono"> .env.local</span> after deploying.
      </div>
    );
  }

  if (isLoading) return <div className="empty">Reading the ledger…</div>;
  if (isError) return <div className="notice err">{error?.message ?? 'Failed to read proposals.'}</div>;

  const proposals = (data ?? []) as readonly Proposal[];
  if (proposals.length === 0) {
    return <div className="empty">No proposals yet. Author the first one above.</div>;
  }

  return (
    <div className="cards">
      {proposals.map((p, i) => (
        <ProposalCard key={`${p.callHash}-${i}`} proposal={p} index={i} />
      ))}
    </div>
  );
}

import { ConnectButton } from '@/components/ConnectButton';
import { ProposeForm } from '@/components/ProposeForm';
import { ProposalList } from '@/components/ProposalList';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { shorten } from '@/lib/format';

export default function Home() {
  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand">
          <span className="mark">
            THE&nbsp;<em>CHAMBER</em>
          </span>
          <span className="net">Paseo Hub · testnet</span>
        </div>
        <ConnectButton />
      </header>

      <section className="hero">
        <h1>
          Author referenda <span className="ital">together</span>, submit them on-chain.
        </h1>
        <p>
          A multisig on Polkadot Hub. Propose an OpenGov referendum by its preimage hash, gather
          approvals from a fixed set of signers, then finalize — the contract dispatches{' '}
          <span className="mono">Referenda::submit</span> as its own sovereign account through the XCM
          precompile.
        </p>
        {CONTRACT_CONFIGURED && (
          <p className="mono" style={{ fontSize: 12, marginTop: 14 }}>
            contract <span style={{ color: 'var(--magenta-ink)' }}>{shorten(CONTRACT_ADDRESS, 10, 8)}</span>
          </p>
        )}
      </section>

      <div className="section-label">Draft a proposal</div>
      <ProposeForm />

      <div className="section-label">The ledger</div>
      <ProposalList />

      <footer className="foot">
        Finalize is payable: it forwards ~10 PAS as the referendum SubmissionDeposit, since the submit
        is dispatched from the contract&apos;s sovereign account.
        <br />
        Note: on Paseo, runtime constants (pallet indices, deposit, governance track) differ from
        Polkadot Hub, which this contract is pinned to — finalize exercises the path but won&apos;t
        produce a correct referendum here.
      </footer>
    </main>
  );
}

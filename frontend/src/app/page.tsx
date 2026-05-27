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
          Author and submit referenda <span className="ital">together</span>.
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
    </main>
  );
}

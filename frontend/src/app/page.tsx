import { ConnectButton } from '@/components/ConnectButton';
import { ProposeForm } from '@/components/ProposeForm';
import { ProposalList } from '@/components/ProposalList';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { explorerAddressUrl } from '@/lib/chain';
import { shorten } from '@/lib/format';

export default function Home() {
  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand">
          <span className="mark">
            THE&nbsp;<em>CHAMBER</em>
          </span>
        </div>
        <div className="topbar-right">
          <span className="net">Paseo Hub</span>
          <ConnectButton />
        </div>
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
        {CONTRACT_CONFIGURED &&
          (() => {
            const url = explorerAddressUrl(CONTRACT_ADDRESS);
            const short = shorten(CONTRACT_ADDRESS, 10, 8);
            return (
              <p className="mono" style={{ fontSize: 12, marginTop: 14 }}>
                contract{' '}
                {url ? (
                  <a href={url} target="_blank" rel="noreferrer" className="addr-link">
                    {short}
                  </a>
                ) : (
                  <span style={{ color: 'var(--magenta-ink)' }}>{short}</span>
                )}
              </p>
            );
          })()}
      </section>

      <div className="section-label">Draft a proposal</div>
      <ProposeForm />

      <div className="section-label">The ledger</div>
      <ProposalList />
    </main>
  );
}

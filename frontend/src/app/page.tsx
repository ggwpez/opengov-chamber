import { ConnectButton } from '@/components/ConnectButton';
import { ProposeForm } from '@/components/ProposeForm';
import { ProposalList } from '@/components/ProposalList';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { chain, explorerAddressUrl } from '@/lib/chain';

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
          <span className="net">{chain.name}</span>
          <ConnectButton />
        </div>
      </header>

      <section className="hero">
        <h1>
          Author and submit referenda <span className="ital">together</span>.
        </h1>
        <p>
          The Chamber is a staging area for OpenGov referenda on Polkadot Hub. Draft, approve, and submit a
          referendum - all from one{' '}
          {(() => {
            const url = CONTRACT_CONFIGURED ? explorerAddressUrl(CONTRACT_ADDRESS) : null;
            return url ? (
              <a href={url} target="_blank" rel="noreferrer" className="addr-link">
                contract
              </a>
            ) : (
              'contract'
            );
          })()}{' '}
          and submitted via XCM.
        </p>
      </section>

      <div className="section-label">Draft a proposal</div>
      <ProposeForm />

      <div className="section-label">The ledger</div>
      <ProposalList />
    </main>
  );
}

import { ConnectButton } from '@/components/ConnectButton';
import { ProposeForm } from '@/components/ProposeForm';
import { ProposalList } from '@/components/ProposalList';
import { CONTRACT_ADDRESS, CONTRACT_CONFIGURED } from '@/lib/contract';
import { chain, explorerAddressUrl } from '@/lib/chain';

const REPO_URL = 'https://github.com/ggwpez/opengov-chamber';

// Inlined at build time by next.config.mjs. Formatted from the ISO string by
// slicing (no Date locale methods) so the static prerender and any future
// client render are byte-identical — no hydration drift.
const COMMIT_SHA = process.env.NEXT_PUBLIC_COMMIT_SHA || '';
const BUILD_TIME = process.env.NEXT_PUBLIC_BUILD_TIME || '';
const builtAt = BUILD_TIME ? `${BUILD_TIME.slice(0, 10)} ${BUILD_TIME.slice(11, 16)} UTC` : null;

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

      <footer className="sitefoot">
        <a href={REPO_URL} target="_blank" rel="noreferrer" className="sitefoot-link">
          Source on GitHub{COMMIT_SHA ? ` · ${COMMIT_SHA}` : ''}
        </a>
        {builtAt ? <span className="sitefoot-meta">Deployed {builtAt}</span> : null}
      </footer>
    </main>
  );
}

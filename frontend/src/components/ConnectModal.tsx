'use client';

import { useEffect, useState } from 'react';
import { useAccount, useConnect, useConnectors, type Connector } from 'wagmi';
import { useActiveAccount } from '@/lib/activeAccount';
import { paseoHub } from '@/lib/chain';
import { shorten } from '@/lib/format';

type Address = `0x${string}`;

/**
 * Unified connect / switch dialog: wallet at the top, account at the bottom.
 *
 * Both sections pre-select a default — the wallet you're already on (or the
 * first detected), and the account currently active (or the wallet's primary).
 * Picking a wallet runs `wallet_requestPermissions` first so the wallet always
 * re-presents its account chooser, even on a reconnect where it would otherwise
 * silently reuse the last account.
 */
export function ConnectModal({ onClose }: { onClose: () => void }) {
  const connectors = useConnectors();
  const { connectAsync, isPending } = useConnect();
  const { connector: active, addresses, isConnected, chainId } = useAccount();
  const { activeAddress, selectAccount } = useActiveAccount();

  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // De-duplicate connectors by name (EIP-6963 discovery can surface a wallet
  // both as a discovered provider and via the generic "injected" fallback).
  const wallets = dedupeConnectors(connectors);

  // Close on Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  async function pickWallet(connector: Connector) {
    setError(null);
    setBusyId(connector.uid);
    try {
      // Force the wallet's own account picker so the user can authorize/switch
      // accounts. Wallets that don't implement it just fall through to connect.
      try {
        const provider = (await connector.getProvider()) as
          | { request?: (args: { method: string; params?: unknown[] }) => Promise<unknown> }
          | undefined;
        await provider?.request?.({
          method: 'wallet_requestPermissions',
          params: [{ eth_accounts: {} }],
        });
      } catch {
        /* unsupported or rejected — connect will still prompt as needed */
      }
      await connectAsync({ connector });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to connect');
    } finally {
      setBusyId(null);
    }
  }

  const accountList = (addresses ?? []) as readonly Address[];

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Connect wallet"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <h2>Connect</h2>
          <button className="modal-x" aria-label="Close" onClick={onClose}>
            ✕
          </button>
        </div>

        {/* ── Wallet ─────────────────────────────────────────── */}
        <div className="modal-label">Wallet</div>
        <div className="picker">
          {wallets.length === 0 && (
            <div className="notice warn">No injected wallet detected. Install MetaMask or Talisman.</div>
          )}
          {wallets.map((c) => {
            const isActive = isConnected && active?.uid === c.uid;
            return (
              <button
                key={c.uid}
                className={`pick-row ${isActive ? 'is-default' : ''}`}
                disabled={isPending}
                onClick={() => pickWallet(c)}
              >
                <span className="pick-main">
                  {c.icon && <img className="pick-icon" src={c.icon} alt="" />}
                  <span>{c.name}</span>
                </span>
                {busyId === c.uid ? (
                  <span className="pick-tag">connecting…</span>
                ) : isActive ? (
                  <span className="pick-tag default">connected</span>
                ) : null}
              </button>
            );
          })}
        </div>

        {/* ── Account ────────────────────────────────────────── */}
        <div className="modal-label">Account</div>
        <div className="picker">
          {!isConnected ? (
            <div className="picker-empty">Choose a wallet to list its accounts.</div>
          ) : accountList.length === 0 ? (
            <div className="picker-empty">No authorized accounts.</div>
          ) : (
            accountList.map((addr) => {
              const isActive = activeAddress?.toLowerCase() === addr.toLowerCase();
              return (
                <button
                  key={addr}
                  className={`pick-row ${isActive ? 'is-default' : ''}`}
                  onClick={() => selectAccount(addr)}
                >
                  <span className="pick-main mono">{shorten(addr, 10, 8)}</span>
                  {isActive && <span className="pick-tag default">active</span>}
                </button>
              );
            })
          )}
        </div>

        {isConnected && chainId !== paseoHub.id && (
          <div className="notice warn">Wallet is not on {paseoHub.name} — switch network to transact.</div>
        )}
        {error && <div className="notice err">{error}</div>}

        <div className="modal-foot">
          <button className="btn btn-primary" onClick={onClose} disabled={!isConnected}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

function dedupeConnectors(connectors: readonly Connector[]): Connector[] {
  const seen = new Set<string>();
  const out: Connector[] = [];
  for (const c of connectors) {
    const key = c.name.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(c);
  }
  return out;
}

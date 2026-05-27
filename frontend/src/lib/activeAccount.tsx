'use client';

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react';
import { useAccount } from 'wagmi';

type Address = `0x${string}`;

type ActiveAccount = {
  /** Every account the connected wallet has authorized for this site. */
  addresses: readonly Address[];
  /** The account the app signs/reads as — the user's pick, else the wallet's primary. */
  activeAddress: Address | undefined;
  /** Pin a specific authorized account as active (persisted per wallet). */
  selectAccount: (addr: Address) => void;
};

const ActiveAccountContext = createContext<ActiveAccount | null>(null);

const STORAGE_PREFIX = 'chamber.activeAccount';

/**
 * Tracks which of the wallet's authorized accounts the app acts as.
 *
 * wagmi has no "set active account" for injected wallets — `useAccount().address`
 * is always the wallet's primary (`addresses[0]`). To let the user act as a
 * different authorized account without changing it inside the wallet, we keep
 * the choice here and pass it as `account` to each `writeContract`. The pick is
 * persisted per connector so it survives reloads, and falls back to the primary
 * whenever the stored account is no longer authorized.
 */
export function ActiveAccountProvider({ children }: { children: ReactNode }) {
  const { address, addresses, connector } = useAccount();
  const [selected, setSelected] = useState<Address>();

  const list = (addresses ?? []) as readonly Address[];
  const storageKey = connector ? `${STORAGE_PREFIX}:${connector.uid}` : null;

  // Restore the remembered account whenever the connected wallet changes.
  useEffect(() => {
    if (!storageKey) {
      setSelected(undefined);
      return;
    }
    const saved = window.localStorage.getItem(storageKey) as Address | null;
    setSelected(saved ?? undefined);
  }, [storageKey]);

  const selectAccount = useCallback(
    (addr: Address) => {
      setSelected(addr);
      if (storageKey) window.localStorage.setItem(storageKey, addr);
    },
    [storageKey],
  );

  const activeAddress =
    selected && list.some((a) => a.toLowerCase() === selected.toLowerCase())
      ? selected
      : address;

  return (
    <ActiveAccountContext.Provider value={{ addresses: list, activeAddress, selectAccount }}>
      {children}
    </ActiveAccountContext.Provider>
  );
}

export function useActiveAccount(): ActiveAccount {
  const ctx = useContext(ActiveAccountContext);
  if (!ctx) throw new Error('useActiveAccount must be used within ActiveAccountProvider');
  return ctx;
}

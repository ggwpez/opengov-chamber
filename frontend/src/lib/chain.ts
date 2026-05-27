import { defineChain } from 'viem';

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ?? 'https://eth-rpc-testnet.polkadot.io/';

/**
 * Optional block explorer base URL (e.g. https://blockscout-testnet.polkadot.io).
 * Left unset → no explorer links anywhere. Trailing slash stripped so callers
 * can append `/address/…` or `/tx/…` cleanly.
 */
export const EXPLORER_URL = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace(/\/$/, '') || undefined;

/** Build an explorer link for an address, or `undefined` if no explorer is set. */
export function explorerAddressUrl(address: string): string | undefined {
  return EXPLORER_URL ? `${EXPLORER_URL}/address/${address}` : undefined;
}

/**
 * Polkadot Hub TestNet (Paseo) — the eth-rpc compatibility layer in front of
 * Asset Hub's `pallet-revive`. Chain id and endpoint per the Polkadot
 * "Connect to Polkadot" docs.
 *
 * Note: the native token is presented with 18 decimals over eth-rpc even though
 * Paseo's native denomination (PAS) is 10 decimals on the Substrate side.
 */
export const paseoHub = defineChain({
  id: 420420417,
  name: 'Polkadot Hub TestNet',
  nativeCurrency: { name: 'Paseo', symbol: 'PAS', decimals: 18 },
  rpcUrls: {
    default: { http: [RPC_URL] },
  },
  // Only declare an explorer when NEXT_PUBLIC_EXPLORER_URL is set; otherwise
  // tooling (and our own UI) should render no outbound links.
  ...(EXPLORER_URL
    ? { blockExplorers: { default: { name: 'Blockscout', url: EXPLORER_URL } } }
    : {}),
  testnet: true,
});

import { defineChain } from 'viem';

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ?? 'https://eth-rpc-testnet.polkadot.io/';

// Chain identity, all from build-time env (defaults match Paseo testnet). The
// native token is presented with 18 decimals over eth-rpc even though the
// Substrate-side denominations differ.
const CHAIN_ID = Number(process.env.NEXT_PUBLIC_CHAIN_ID ?? '420420417');
const CHAIN_NAME = process.env.NEXT_PUBLIC_CHAIN_NAME ?? 'Polkadot Hub TestNet';
const CHAIN_SYMBOL = process.env.NEXT_PUBLIC_CHAIN_SYMBOL ?? 'PAS';
const CHAIN_TESTNET = process.env.NEXT_PUBLIC_CHAIN_TESTNET !== 'false';

/**
 * Optional block explorer base URL (e.g. https://assethub-paseo.subscan.io).
 * Left unset → no explorer links anywhere. Trailing slash stripped so callers
 * can append `/account/…` or `/preimage/…` cleanly.
 */
export const EXPLORER_URL = process.env.NEXT_PUBLIC_EXPLORER_URL?.replace(/\/$/, '') || undefined;

/** Build an explorer link for an account address, or `undefined` if no explorer is set. */
export function explorerAddressUrl(address: string): string | undefined {
  return EXPLORER_URL ? `${EXPLORER_URL}/account/${address}` : undefined;
}

/** Build an explorer link for a preimage / call hash, or `undefined` if no explorer is set. */
export function explorerPreimageUrl(hash: string): string | undefined {
  return EXPLORER_URL ? `${EXPLORER_URL}/preimage/${hash}` : undefined;
}

/**
 * The eth-rpc compatibility layer in front of a `pallet-revive` Hub. Identity
 * (id / name / symbol / testnet / rpc) is read from build-time env, defaulting
 * to Polkadot Hub TestNet (Paseo) — see the env vars at the top of this file.
 */
export const chain = defineChain({
  id: CHAIN_ID,
  name: CHAIN_NAME,
  nativeCurrency: { name: CHAIN_SYMBOL, symbol: CHAIN_SYMBOL, decimals: 18 },
  rpcUrls: {
    default: { http: [RPC_URL] },
  },
  // Only declare an explorer when NEXT_PUBLIC_EXPLORER_URL is set; otherwise
  // tooling (and our own UI) should render no outbound links.
  ...(EXPLORER_URL
    ? { blockExplorers: { default: { name: 'Subscan', url: EXPLORER_URL } } }
    : {}),
  testnet: CHAIN_TESTNET,
});

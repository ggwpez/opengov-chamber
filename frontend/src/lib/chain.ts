import { defineChain } from 'viem';

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ?? 'https://eth-rpc-testnet.polkadot.io/';

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
  blockExplorers: {
    // Adjust if you prefer Routescan; only used to build outbound links.
    default: {
      name: 'Blockscout',
      url: 'https://blockscout-passet-hub.parity-testnet.parity.io',
    },
  },
  testnet: true,
});

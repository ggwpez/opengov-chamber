import { createConfig, http } from 'wagmi';
import { injected } from 'wagmi/connectors';
import { paseoHub } from './chain';

/**
 * wagmi config. We only wire the injected connector (MetaMask / any EIP-1193
 * wallet) — that's all the Hub eth-rpc flow needs. `ssr: true` keeps Next's
 * server render from touching `window`.
 */
export const wagmiConfig = createConfig({
  chains: [paseoHub],
  connectors: [injected()],
  transports: {
    [paseoHub.id]: http(),
  },
  ssr: true,
});

declare module 'wagmi' {
  interface Register {
    config: typeof wagmiConfig;
  }
}

# The Chamber — frontend

A Next.js + wagmi/viem dApp for the OpenGov multisig-proposal PVM contract, targeting
**Polkadot Hub TestNet (Paseo)**. Connect a wallet, draft a proposal by its preimage
hash, gather approvals, and finalize (which dispatches `Referenda::submit` via the XCM
precompile).

## 1. Deploy the contract

The contract is a PolkaVM blob, not EVM bytecode — but `pallet-revive`'s eth-rpc accepts
it as create-transaction init code, so standard Ethereum tooling works.

```bash
# from the repo root
just build                                  # produces target/contract.release.polkavm

export ETH_RPC_URL=https://eth-rpc-testnet.polkadot.io/
export PRIVATE_KEY=0x...                     # funded with PAS (Paseo faucet)
cast send --private-key "$PRIVATE_KEY" --create \
  "$(xxd -p -c 99999 target/contract.release.polkavm)" --json
```

`deploy()` takes no constructor args, so the raw blob hex is the entire init code. Record
the `contractAddress` from the output.

> Account note: an Ethereum-only key gets a deterministic fallback Substrate AccountId in
> `pallet-revive`, but it must hold PAS for gas/storage. Some flows also want a one-time
> `pallet_revive::map_account`.

## 2. Configure & run

```bash
cd frontend
cp .env.example .env.local        # set NEXT_PUBLIC_CONTRACT_ADDRESS to the deployed address
npm install
npm run dev                       # http://localhost:3000
```

## 3. Tests

```bash
npm test          # Vitest: unit (lib) + component (gating) tests — no wallet/node needed
npm run test:watch
npm run typecheck # tsc --noEmit, also covers the test files
npm run e2e        # Playwright smoke test (boots `next dev` itself, headless Chromium)
```

First-time E2E only: `npx playwright install chromium`.

Three layers, all runnable without a wallet or a deployed contract:

- **Unit** (`src/lib/*.test.ts`). `format.test.ts` covers the address/hash validators and
  the blake2-256 `callHashFromHex` derivation. `proposalKey.test.ts` is the important one:
  it pins the client-side key derivation to the **same golden hash** the Rust suite asserts
  (`../tests/tests/proposal_key.rs::proposal_key_golden_hash`), using the identical fixture.
  If `proposalKey.ts` ever drifts from `contract/src/lib.rs::proposal_key`, approve/finalize
  would silently key a non-existent proposal and revert — this test fails first.
- **Component** (`src/components/*.test.tsx`, Vitest + Testing Library, wagmi mocked).
  `ProposalCard.test.tsx` asserts the action-button gating — Approve only for un-signed
  listed approvers, Finalize for the creator once `approvedBy ≥ minApprovers` (forwarding the
  deposit as `value`), Cancel for the creator, and **no buttons** on terminal
  (Submitted/Closed) proposals — plus that each click calls `writeContract` with the right
  function and args. `ProposeForm.test.tsx` covers the validation gate (creator-as-approver,
  min > approver count, wrong-network switch) and the `propose` call args.
- **E2E** (`e2e/smoke.spec.ts`, Playwright). Boots the dev server and asserts the
  disconnected shell hydrates without throwing. Wallet-driven flows aren't simulated — those
  are the component layer's job; see the caveats below for why a real `finalize` E2E on Paseo
  wouldn't be meaningful anyway.

## Notes / caveats

- **`finalize` is payable** — it forwards ~10 PAS (the `SubmissionDeposit`, EVM-denominated
  as `parseEther("10")`), because the submit runs from the contract's sovereign account.
- **`propose` wants a Substrate preimage hash + length.** The "I have the call bytes" tab
  derives them (blake2-256 + byte length) from pasted SCALE call hex, but the preimage
  itself still has to be noted on Asset Hub (via PAPI / polkadot-js) for a referendum to
  resolve it — that's out of scope for this EVM dApp.
- **Proposal keys are recomputed client-side** (`src/lib/proposalKey.ts`) because
  `allProposals()` returns structs without their storage keys, yet approve/finalize are
  keyed by that hash. It mirrors `contract/src/lib.rs::proposal_key` byte-for-byte — keep
  the two in sync.
- **Paseo ≠ Polkadot Hub constants.** This contract is pinned to Polkadot Hub's pallet
  indices / deposit / governance track, so `finalize` exercises the path on Paseo but won't
  produce a correct referendum. Use it here for propose/approve/deploy mechanics.

## Stack

Next.js 14 (App Router) · wagmi v2 · viem v2 · @tanstack/react-query · blakejs.
ABI is hand-authored in `src/lib/abi.ts` from `../contract/Contract.sol` (no solc needed).

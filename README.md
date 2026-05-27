# OpenGov Proposal Multisig

A smart contract — and dApp — for **collectively submitting OpenGov referenda** on
Polkadot Hub (Asset Hub). A fixed set of signers co-authors a referendum off-chain, gathers
on-chain approvals, and once the threshold is met the contract submits the referendum
**itself**, as its own sovereign account, through Asset Hub's XCM precompile.

It's a PolkaVM (PVM) contract written in Rust against `pallet-revive`, exposing a Solidity
ABI so any Ethereum tooling (and the bundled frontend) can talk to it.

## How it works

1. **Propose** — anyone registers a proposal: the preimage `callHash` + `callLen` of the
   runtime call to enact, an enactment delay, an ordered set of `approvers`, and a
   `minApprovers` threshold. The proposal starts in status **`Review`**.
2. **Approve** — each listed approver signs once. Approvals are tracked on-chain. Only
   possible while the proposal is in `Review`.
3. **Finalize** — once `approvedBy ≥ minApprovers`, **the creator** finalizes. The contract
   `api::call`s the **XCM precompile** with a local `Transact` wrapping
   `Referenda::submit`, dispatched under the contract's own signed origin, and the proposal
   moves to status **`Submitted`**.
4. **Close** — alternatively, **the creator** can abandon a proposal before finalizing,
   moving it to status **`Closed`**. Not possible after finalizing.

Because the submit runs as the contract's sovereign account, `finalize()` is **payable**:
the caller must forward the referendum `SubmissionDeposit` (~10 DOT, EVM-denominated) as
value, which `pallet-revive` credits to the contract before the dispatch.

## Proposal lifecycle / status

Every `Proposal` carries a `ProposalStatus status` field (a Solidity `enum`, ABI-encoded as
`uint8`):

| Value | Variant | Meaning |
|---|---|---|
| `0` | `Review` | Just proposed; collecting approvals. Can be approved, finalized, or closed. |
| `1` | `Submitted` | `finalize` ran and the referendum was dispatched. **Terminal.** |
| `2` | `Closed` | `close` ran before finalizing; abandoned. **Terminal.** |

```
         approve (creator-listed approvers, stays in Review)
         ┌────┐
         ▼    │
  propose ──▶ Review ──finalize──▶ Submitted   (terminal)
                 │
                 └──── close ─────▶ Closed      (terminal)
```

Both terminal states reject every further action: `approve`, `finalize`, and `close` all
revert once a proposal has left `Review`. Guard rules enforced on-chain:

- `approve` — caller must be a listed approver, not already recorded, **and** status must be
  `Review`.
- `finalize` — caller must be the **creator**, threshold met, **and** status `Review`.
- `close` — caller must be the **creator** **and** status `Review`.

**`close` no longer deletes the proposal** — it retains it with `status = Closed`, so closed
proposals still appear in `proposal(hash)` and `allProposals()`. (Refunds are unrelated to
status: `refund()` pays back the caller's accumulated deposit tally and is independent of any
proposal's state.)

## UI work needed (for the next agent)

The on-chain change above is done and tested; the frontend (`frontend/`) still needs to catch
up:

1. **Regenerate the ABI / types.** `Proposal` gained a trailing `status` field and there's a
   new `ProposalStatus` enum — any decoded proposal now has an extra `uint8`. Re-export the
   ABI from `Contract.sol` / rebuild whatever the frontend consumes.
2. **Surface the status** on each proposal (badge: Review / Submitted / Closed).
3. **Gate the action buttons by status** (all only enabled in `Review`):
   - Approve → only for listed approvers who haven't approved yet.
   - Finalize → creator only, once `approvedBy ≥ minApprovers`.
   - Close → creator only.
   Disable/hide them for `Submitted` and `Closed`, since the contract will revert.
4. **Show closed proposals** — `allProposals()` now includes `Closed` ones (they're no longer
   deleted); consider a filter or visually de-emphasizing them.
5. There's a `Closed` event (`event Closed(bytes32 indexed proposalHash)`) alongside the
   existing `Finalized`/`Approved`/etc. if the UI subscribes to events.

## Layout

| Path | What |
|---|---|
| `contract/` | The Rust PVM contract. `Contract.sol` is the ABI; `src/contract.rs` the entry points; `src/xcm.rs` builds the XCM `Transact` calldata. |
| `tests/` | Integration tests against the **real** `asset-hub-polkadot-runtime` (the XCM path actually executes), plus `proposal_key` golden-hash and XCM byte-equality tests. |
| `frontend/` | Next.js + wagmi/viem dApp — connect a wallet, propose/approve/finalize, browse the ledger. See `frontend/README.md`. |
| `Justfile` | Build, test, deploy, and frontend recipes. |

## Quickstart

```bash
just build              # compile the PVM blob -> target/contract.release.polkavm
just test               # run the integration + unit tests

cp .env.example .env    # set PRIVATE_KEY (funded with PAS for Paseo)
just account            # show eth + substrate address and balance
just deploy             # build + balance check + deploy to Paseo testnet
just frontend-env 0x…   # point the dApp at the deployed address
just dev                # start the frontend at http://localhost:3000
```

`just deploy-polkadot` / `deploy-kusama` target the other Hub networks.

## Caveats

- **Pinned to Polkadot Hub.** Pallet indices, the governance track, and the deposit are
  hardcoded to Asset Hub Polkadot. `propose`/`approve` work on any Hub, but `finalize`
  only produces a *correct* referendum on Polkadot Hub mainnet — on Paseo it exercises the
  path without a meaningful submission.
- **Preimage noting is out of scope** for the contract/dApp. `propose` only records the
  call's hash + length; the preimage itself must be noted on Asset Hub (via PAPI /
  polkadot-js) for a referendum to resolve it.
- **Open placeholders** remain in the XCM module (proposal origin track, fallback weights);
  see the inline `TODO`s in `contract/src/xcm.rs`.

## Stack

Rust `no_std` · `pallet-revive` / PolkaVM · `alloy-core` (Solidity ABI) · XCM v5 · Next.js +
wagmi + viem.

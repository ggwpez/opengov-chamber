# The Chamber

A contract — and dApp — for submitting OpenGov referenda **collectively**.

A fixed set of signers co-authors a referendum off-chain and gathers approvals on-chain. Once
the threshold is met, the contract submits the referendum itself, through Asset Hub's XCM
precompile.

Under the hood: a PolkaVM contract in Rust on `pallet-revive`, with a Solidity ABI on top — so
any Ethereum tooling, and the bundled dApp, can talk to it.

## How it works

1. **Propose.** Anyone registers a proposal: the preimage `callHash` + `callLen`, an enactment
   moment (`DispatchTime` — `At` an absolute block or `After` a delay), a `track` (`Root` or
   `WhitelistedCaller`), an ordered `approvers` set, and a `minApprovers` threshold. It starts
   in **`Review`**.
2. **Approve.** Each listed approver signs once. Tracked on-chain. Only while in `Review`.
3. **Finalize.** Once `approvedBy ≥ minApprovers`, the **creator** finalizes. The contract
   `api::call`s the **XCM precompile** with a local `Transact` wrapping `Referenda::submit`,
   dispatched as the contract itself — not the caller. The stored fields become the referendum:
   `callHash` + `callLen` → the `Bounded::Lookup` preimage reference, `enactment` → the
   `DispatchTime` moment (`At` / `After`), `track` → the `proposal_origin` (`Root` →
   `system(Root)`, `WhitelistedCaller` → `Origins(WhitelistedCaller)`). Status → **`Submitted`**.
4. **Close.** Or the **creator** abandons it before finalizing. Status → **`Closed`**. Not
   possible once finalized.

`finalize()` is **payable**. The deposit is charged to the contract, so the caller forwards the
referendum `SubmissionDeposit` (~10 DOT, EVM-denominated) as value; `pallet-revive` credits it
to the contract before dispatch.

## Lifecycle

Every `Proposal` carries a `ProposalStatus status` — a Solidity `enum`, ABI-encoded as `uint8`:

| Value | Variant | Meaning |
|---|---|---|
| `0` | `Review` | Just proposed; collecting approvals. Can be approved, finalized, or closed. |
| `1` | `Submitted` | `finalize` ran; the referendum was dispatched. **Terminal.** |
| `2` | `Closed` | `close` ran before finalizing; abandoned. **Terminal.** |

```
         approve (listed approvers, stays in Review)
         ┌────┐
         ▼    │
  propose ──▶ Review ──finalize──▶ Submitted   (terminal)
                 │
                 └──── close ─────▶ Closed      (terminal)
```

Terminal states reject everything. `approve`, `finalize`, and `close` all revert once a
proposal leaves `Review`. The on-chain guards:

- **`approve`** — caller is a listed approver, not already recorded, status `Review`.
- **`finalize`** — caller is the **creator**, threshold met, status `Review`.
- **`close`** — caller is the **creator**, status `Review`.

`close` keeps the proposal (`status = Closed`); it no longer deletes it. Closed proposals still
show up in `proposal(hash)` and `allProposals()`. Refunds are orthogonal: `refund()` pays back
the caller's accumulated deposit tally, independent of any proposal's state.

## Frontend

`frontend/` is a Next.js + wagmi/viem dApp: connect a wallet, propose / approve / finalize,
browse the ledger. It mirrors the contract one-to-one.

- **ABI in sync.** `src/lib/abi.ts` carries the `status` field, the `close` / `refund` /
  `deposits` functions, the `Closed` / `Refunded` events, and the errors. A `ProposalStatus`
  enum ships alongside the `Proposal` type.
- **Status badge** per card — `collecting` / `ready to submit` in `Review`, then `submitted` or
  `cancelled`.
- **Actions gated by status.** Approve / Finalize / Cancel render only in `Review`; terminal
  proposals show no buttons. Approve is for listed approvers who haven't voted; Finalize is
  creator-only, enabled at threshold; Cancel (`close`) is creator-only.
- **Split lists.** The open `Review` queue sits apart from terminal proposals, which collapse
  under a de-emphasized "Submitted & cancelled" heading.

See `frontend/README.md`.

## Storage encoding

A single `pallet-revive` storage value caps at **416 bytes** (`limits::STORAGE_BYTES`). The
Solidity ABI pads every head field to a full 32-byte word, so a `Proposal` round-trips through
alloy at `352 + 32·(N + M)` bytes — and hits the cap at just **N + M = 2** (`N = approvers`,
`M = approvedBy`).

So the contract carries a hand-rolled packed codec (`contract/src/codec.rs`, mirrored in
`frontend/src/lib/proposalCodec.ts`) for the bytes that actually hit `api::set_storage`. The
Solidity ABI is still the wire format on every entrypoint — `propose` / `proposal` /
`allProposals` / events — so wagmi/viem decode normally. Only the on-storage blob is packed.

### Wire format

```
 off  size  field
   0   1    version (= 0x02)
   1   32   callHash
  33   4    callLen                (LE u32)
  37   1    enactment.kind         (0=At, 1=After)
  38   4    enactment.block        (LE u32)
  42   1    track                  (0=Root, 1=WhitelistedCaller)
  43   20   creator
  63   1    N = approvers.len()
  ..  20·N  approvers
   .   1    minApprovers           (≤ N, fits in u8)
   .   1    M = approvedBy.len()   (≤ N)
   .  20·M  approvedBy
   .   1    status                 (0=Review, 1=Submitted, 2=Closed)
```

Total = **67 + 20·(N + M)** bytes. The leading `version` byte partitions the keyspace cleanly
across incompatible format changes — bumped to `0x02` when `enactment` (kind + block) and
`track` replaced the bare `enactmentDelay` u32 of `0x01`.

The Rust and TS encoders are pinned to each other by **golden vectors** — both decode to the
same 127-byte buffer (`tests/tests/codec.rs::golden_vector`,
`frontend/src/lib/proposalCodec.test.ts`). Cross-language drift fails in CI, not silently
on-chain.

### `proposalKey` and the identity prefix

`encode_identity(prop)` is a strict prefix of `encode(prop)`: every field that gives a proposal
its identity (`callHash`, `callLen`, `enactment`, `track`, `creator`, `approvers`,
`minApprovers`) — and none of the mutable bits (`approvedBy`, `status`). The key is then:

```rust
keccak256(b"Proposal:" || codec::encode_identity(prop))
```

One byte-level encoding, shared with storage, kept in sync across both languages. The
frontend's `proposalKey` is a ~5-line wrapper over `encodeIdentity`. The strict-prefix property
is asserted on both sides.

### Capacity: `MAX_APPROVERS = 8`

Worst case is every approver having voted (`M = N`). At `N = 8` the blob is
`67 + 20·(8+8) = 387` bytes — **29 under the cap**:

| N | M | size  | fits |
|---|---|-------|------|
| 8 | 8 | 387 B | ✓ (29 B headroom) |
| 9 | 8 | 407 B | ✓ (9 B headroom) |
| 9 | 9 | 427 B | ✗ |

So `8` is the largest `approvers.len()` for which *every* approval is guaranteed to round-trip.
The constant is hardcoded in both `codec.rs` and `proposalCodec.ts`;
`max_approvers_with_full_approval_fits_storage_cap` fails loudly if a layout change breaks it.
Growing past 8 means shaving bytes — drop the version byte, fold `status` into spare bits, or
move `approvedBy` to its own storage key.

## Layout

| Path | What |
|---|---|
| `contract/` | The Rust PVM contract. `Contract.sol` is the ABI; `src/contract.rs` the entry points; `src/xcm.rs` builds the XCM `Transact` calldata. |
| `tests/` | Integration tests against the **real** `asset-hub-polkadot-runtime` (the XCM path executes), plus `proposal_key` golden-hash and XCM byte-equality tests. |
| `frontend/` | The Next.js + wagmi/viem dApp. See `frontend/README.md`. |
| `Justfile` | Build, test, deploy, frontend recipes. |

## Quickstart

```bash
just build              # compile the PVM blob -> target/contract.release.polkavm
just test               # integration + unit tests

cp .env.example .env    # set PRIVATE_KEY (funded with PAS for Paseo)
just account            # eth + substrate address and balance
just deploy             # build + balance check + deploy to Paseo testnet
just frontend-env 0x…   # point the dApp at the deployed address
just dev                # frontend at http://localhost:3000
```

`just deploy-polkadot` / `deploy-kusama` target the other Hubs.

## Caveats

- **Pinned to Polkadot Hub.** Pallet indices, track origins, and the deposit are hardcoded to
  Asset Hub Polkadot. `propose` / `approve` work on any Hub; `finalize` only produces a
  *correct* referendum on Polkadot Hub mainnet. On Paseo it exercises the path without a
  meaningful submission.
- **Preimage noting is out of scope.** `propose` records only the call's hash + length. The
  preimage itself must be noted on Asset Hub (PAPI / polkadot-js) for a referendum to resolve.
- **Open placeholders** remain in the XCM module — fallback weights for the inner `Transact`.
  See the `TODO`s in `contract/src/xcm.rs`.

## Stack

Rust `no_std` · `pallet-revive` / PolkaVM · `alloy-core` (Solidity ABI) · XCM v5 · Next.js +
wagmi + viem.

# OpenGov Proposal Multisig

A smart contract — and dApp — for **collectively submitting OpenGov referenda** on
Polkadot Hub (Asset Hub). A fixed set of signers co-authors a referendum off-chain, gathers
on-chain approvals, and once the threshold is met the contract submits the referendum
**itself**, as its own sovereign account, through Asset Hub's XCM precompile.

It's a PolkaVM (PVM) contract written in Rust against `pallet-revive`, exposing a Solidity
ABI so any Ethereum tooling (and the bundled frontend) can talk to it.

## How it works

1. **Propose** — anyone registers a proposal: the preimage `callHash` + `callLen` of the
   runtime call to enact, an enactment moment (`DispatchTime` — `At` an absolute block or
   `After` a delay), a governance `track` (`Root` or `WhitelistedCaller`), an ordered set of
   `approvers`, and a `minApprovers` threshold. The proposal starts in status **`Review`**.
2. **Approve** — each listed approver signs once. Approvals are tracked on-chain. Only
   possible while the proposal is in `Review`.
3. **Finalize** — once `approvedBy ≥ minApprovers`, **the creator** finalizes. The contract
   `api::call`s the **XCM precompile** with a local `Transact` wrapping `Referenda::submit`,
   dispatched under the contract's own sovereign account. The proposal's stored fields drive
   the submitted referendum: `callHash` + `callLen` become the `Bounded::Lookup` preimage
   reference, `enactment` becomes the `DispatchTime` enactment moment (`At`/`After`), and
   `track` selects the `proposal_origin` (`Root` → `system(Root)`, `WhitelistedCaller` →
   `Origins(WhitelistedCaller)`). The proposal then moves to status **`Submitted`**.
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

## UI support for status & cancel (done)

The on-chain change above is reflected in the frontend (`frontend/`):

1. **ABI / types synced.** `src/lib/abi.ts` carries the trailing `status` field on `Proposal`,
   the `close`/`refund`/`deposits` functions, the `Closed`/`Refunded` events, and the new
   errors. A `ProposalStatus` enum (`Review`/`Submitted`/`Closed`) is exported alongside the
   `Proposal` type.
2. **Status badge** on every card — `collecting`/`ready to submit` while in `Review`,
   `submitted`, or `cancelled`.
3. **Actions gated by status.** Approve/Finalize/Cancel only render while a proposal is in
   `Review`; terminal proposals show no action buttons (the contract would revert):
   - Approve → listed approvers who haven't approved yet.
   - Finalize → creator only, enabled once `approvedBy ≥ minApprovers`.
   - Cancel (`close`) → creator only.
4. **Separate lists.** `ProposalList` splits the open (`Review`) queue from the terminal
   (`Submitted`/`Closed`) ones, which sit under a "Submitted & cancelled" heading and are
   visually de-emphasized so they don't clutter the active queue.

## Storage encoding

A single storage value in `pallet-revive` is capped at **416 bytes**
(`pallet_revive::limits::STORAGE_BYTES`). The Solidity ABI pads every head
field to a full 32-byte EVM word regardless of declared width, so a
`Proposal` round-trips through alloy at `352 + 32·(N + M)` bytes — colliding
with the cap at just **N + M = 2** entries (where `N = approvers`,
`M = approvedBy`).

To buy room, the contract carries a hand-rolled packed codec
(`contract/src/codec.rs`, mirrored in `frontend/src/lib/proposalCodec.ts`)
for the bytes that actually hit `api::set_storage`. The Solidity ABI is still
the wire format on every contract entrypoint — `propose` / `proposal` /
`allProposals` / events — so wagmi/viem decode normally; only the on-storage
blob is packed.

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

Total = **67 + 20·(N + M)** bytes. The leading `version` byte partitions
keyspace cleanly if the format ever changes incompatibly — it was bumped to
`0x02` when `enactment` (kind + block) and `track` replaced the bare
`enactmentDelay` u32 of `0x01`.

The Rust and TS encoders are pinned against each other by **golden vectors**
that decode to the exact same 127-byte buffer (`tests/tests/codec.rs::golden_vector`
and `frontend/src/lib/proposalCodec.test.ts`), so a drift between languages
shows up immediately in CI rather than as silent on-chain corruption.

### `proposalKey` and the identity prefix

The codec exposes `encode_identity(prop)` — a strict prefix of `encode(prop)`
covering every field that gives a proposal its identity (`callHash`,
`callLen`, `enactment`, `track`, `creator`, `approvers`, `minApprovers`), but
*not* the mutable bits (`approvedBy`, `status`). `proposal_key` is then:

```rust
keccak256(b"Proposal:" || codec::encode_identity(prop))
```

Sharing the encoder with storage means there's a single byte-level encoding
to keep in sync across languages; the frontend's `proposalKey` mirror is a
~5-line wrapper around `encodeIdentity`. The strict-prefix property is
asserted by a test on both sides.

### Capacity: `MAX_APPROVERS = 8`

The worst case for a single proposal is every approver having voted
(`M = N`). At `N = 8` the blob is `67 + 20·(8+8) = 387` bytes — **29 bytes
under the cap**. One more approver-or-vote (387 → 407 B) would still fit,
but two more (387 → 427 B) would overflow:

| N | M | size  | fits |
|---|---|-------|------|
| 8 | 8 | 387 B | ✓ (29 B headroom) |
| 9 | 8 | 407 B | ✓ (9 B headroom) |
| 9 | 9 | 427 B | ✗ |

So `MAX_APPROVERS = 8` is the largest `approvers.len()` for which *every*
approval is guaranteed to round-trip through storage. The constant is a
hardcoded literal in both `codec.rs` and `proposalCodec.ts`; the
`max_approvers_with_full_approval_fits_storage_cap` test fails loudly if a
future layout change ever makes 8 no longer fit. To grow past 8 you'd have
to shave bytes off the layout (drop the version byte, fold the status byte
into spare bits, or move `approvedBy` to a separate storage key).

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

- **Pinned to Polkadot Hub.** Pallet indices, the governance track origins, and the deposit
  are hardcoded to Asset Hub Polkadot. `propose`/`approve` work on any Hub, but `finalize`
  only produces a *correct* referendum on Polkadot Hub mainnet — on Paseo it exercises the
  path without a meaningful submission. The proposal's `track` (`Root` / `WhitelistedCaller`)
  selects which hardcoded origin the referendum is submitted under.
- **Preimage noting is out of scope** for the contract/dApp. `propose` only records the
  call's hash + length; the preimage itself must be noted on Asset Hub (via PAPI /
  polkadot-js) for a referendum to resolve it.
- **Open placeholders** remain in the XCM module (fallback weights for the inner `Transact`);
  see the inline `TODO`s in `contract/src/xcm.rs`.

## Stack

Rust `no_std` · `pallet-revive` / PolkaVM · `alloy-core` (Solidity ABI) · XCM v5 · Next.js +
wagmi + viem.

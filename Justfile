# Load PRIVATE_KEY (and any overrides) from a root .env if present.
set dotenv-load := true

host := `rustc -vV | sed -n 's/^host: //p'`

# Compiled PVM blob that gets deployed (produced by `just build`).
blob := "target/contract.release.polkavm"

# pallet-revive eth-rpc endpoints (per the "Connect to Polkadot" docs).
paseo_rpc    := "https://eth-rpc-testnet.polkadot.io/"   # Polkadot Hub TestNet, chain 420420417
polkadot_rpc := "https://eth-rpc.polkadot.io/"           # Polkadot Hub mainnet, chain 420420419
kusama_rpc   := "https://eth-rpc-kusama.polkadot.io/"    # Kusama Hub,           chain 420420418

build:
    cd contract && cargo build --release
b: build

test: build
    cd tests && cargo test -- --nocapture
t: test

# ---------------------------------------------------------------- deployment

# Show the deploying account's address and native balance, fail if it's zero.
account rpc=polkadot_rpc:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cast >/dev/null || { echo "✖ cast not found — install Foundry: https://getfoundry.sh"; exit 1; }
    : "${PRIVATE_KEY:?✖ set PRIVATE_KEY (export it, or put it in a root .env)}"
    case "$PRIVATE_KEY" in
      0x0000000000000000000000000000000000000000000000000000000000000000)
        echo "✖ PRIVATE_KEY is still the .env.example placeholder — put a real, funded key in .env"; exit 1 ;;
    esac
    addr=$(cast wallet address --private-key "$PRIVATE_KEY" 2>/dev/null) || {
      echo "✖ invalid PRIVATE_KEY — expected a 0x-prefixed 32-byte secp256k1 key"; exit 1; }
    # pallet-revive's fallback AccountId for an Ethereum key: the 20-byte H160
    # padded to 32 bytes with 0xEE, SS58-encoded (Polkadot prefix 0, as Asset
    # Hub uses). This holds until the account is explicitly mapped via
    # pallet_revive::map_account.
    acct="0x$(printf '%s' "${addr#0x}" | tr 'A-F' 'a-f')eeeeeeeeeeeeeeeeeeeeeeee"
    wei=$(cast balance "$addr" --rpc-url "{{rpc}}")
    echo "eth address:        $addr"
    if command -v subkey >/dev/null; then
      ss58=$(subkey inspect --public "$acct" --network polkadot 2>/dev/null | awk '/SS58 Address/{print $NF; exit}')
      echo "substrate address:  $ss58"
    fi
    echo "fallback AccountId: $acct"
    echo "rpc:                {{rpc}}"
    echo "balance:            $(cast from-wei "$wei") ($wei wei)"
    if [ "$wei" = "0" ]; then
      echo "✖ zero balance — fund this account before deploying (Paseo faucet for testnet)"
      exit 1
    fi
    echo "✔ account funded"

# `deploy()` takes no constructor args, so the raw blob hex is the whole init
# code. Requires `cast` (Foundry) + `xxd`, and PRIVATE_KEY in the env or a root
# .env file (funded with the chain's native token). Examples:
#   just deploy                  # -> Paseo testnet (default)
#   just deploy-polkadot         # -> Polkadot Hub mainnet

# Deploy the compiled PVM blob to an eth-rpc endpoint (default: Paseo testnet).
deploy rpc=paseo_rpc: build (account rpc)
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cast >/dev/null || { echo "✖ cast not found — install Foundry: https://getfoundry.sh"; exit 1; }
    command -v xxd  >/dev/null || { echo "✖ xxd not found"; exit 1; }
    : "${PRIVATE_KEY:?✖ set PRIVATE_KEY (export it, or put it in a root .env)}"
    test -f "{{blob}}" || { echo "✖ missing {{blob}} — run: just build"; exit 1; }
    echo "▸ deploying {{blob}} ($(wc -c < "{{blob}}" | tr -d ' ') bytes) → {{rpc}}"
    out=$(cast send --private-key "$PRIVATE_KEY" --rpc-url "{{rpc}}" \
            --create "$(xxd -p -c 99999 "{{blob}}")" --json)
    echo "$out"
    if command -v jq >/dev/null; then
      addr=$(echo "$out" | jq -r '.contractAddress // empty')
      if [ -n "$addr" ]; then
        echo ""
        echo "✔ contract deployed at: $addr"
        echo "  → wire it into the frontend: just frontend-env $addr {{rpc}}"
      fi
    fi

# Deploy to Polkadot Hub TestNet (Paseo).
deploy-paseo:    (deploy paseo_rpc)
# Deploy to Polkadot Hub mainnet.
deploy-polkadot: (deploy polkadot_rpc)
# Deploy to Kusama Hub.
deploy-kusama:   (deploy kusama_rpc)

# Bump the account nonce with the cheapest possible tx (a 0-value self-transfer,
# 21000 gas, no calldata) to clear a "Transaction is temporarily banned" (error
# 1012) from the eth-rpc tx pool — usually left behind by a deploy attempted on a
# zero-balance account. The account must be funded first. Examples:
#   just unban                   # -> Paseo testnet (default)
#   just unban-polkadot          # -> Polkadot Hub mainnet
unban rpc=paseo_rpc: (account rpc)
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cast >/dev/null || { echo "✖ cast not found — install Foundry: https://getfoundry.sh"; exit 1; }
    : "${PRIVATE_KEY:?✖ set PRIVATE_KEY (export it, or put it in a root .env)}"
    addr=$(cast wallet address --private-key "$PRIVATE_KEY")
    echo "▸ sending 0-value self-transfer ($addr → itself) to bump the nonce → {{rpc}}"
    cast send "$addr" --value 0 --private-key "$PRIVATE_KEY" --rpc-url "{{rpc}}" --json
    echo "✔ nonce bumped — retry: just deploy {{rpc}}"

# Unban on Polkadot Hub TestNet (Paseo).
unban-paseo:    (unban paseo_rpc)
# Unban on Polkadot Hub mainnet.
unban-polkadot: (unban polkadot_rpc)
# Unban on Kusama Hub.
unban-kusama:   (unban kusama_rpc)

# Read the contract's on-chain `deployer()` — the owner account `destroy` accepts
# and the frontend gates its destroy button on. Read-only (no key needed). The
# address and rpc default to whatever `just frontend-env` wrote into
# frontend/.env.local; override either positionally. Examples:
#   just deployer                                              # use frontend/.env.local
#   just deployer 0xCONTRACT                                   # explicit address
#   just deployer 0xCONTRACT https://eth-rpc.polkadot.io/      # explicit address + rpc
deployer addr="" rpc="":
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cast >/dev/null || { echo "✖ cast not found — install Foundry: https://getfoundry.sh"; exit 1; }
    env=frontend/.env.local
    addr="{{addr}}"
    rpc="{{rpc}}"
    # Fall back to the values `just frontend-env` baked into the frontend env file.
    # Grep the specific keys rather than sourcing it (NEXT_PUBLIC_CHAIN_NAME has a
    # space, so `. .env.local` under `set -e` would try to run it as a command).
    if [ -f "$env" ]; then
      [ -n "$addr" ] || addr=$(grep -E '^NEXT_PUBLIC_CONTRACT_ADDRESS=' "$env" | head -1 | cut -d= -f2-)
      [ -n "$rpc"  ] || rpc=$(grep -E '^NEXT_PUBLIC_RPC_URL=' "$env" | head -1 | cut -d= -f2-)
    fi
    rpc="${rpc:-{{paseo_rpc}}}"
    [ -n "$addr" ] || { echo "✖ no contract address — pass one (just deployer 0x… [rpc]) or run: just frontend-env <addr> <rpc>"; exit 1; }
    echo "▸ deployer() of $addr → $rpc"
    cast call "$addr" "deployer()(address)" --rpc-url "$rpc"

# Tear the contract down via `destroy()`: removes its code + storage and sweeps the
# remaining balance to the deployer. IRREVERSIBLE. Requires PRIVATE_KEY (in the env
# or a root .env) to be the contract's deployer — the call reverts with NotOwner
# otherwise, and with OutstandingDeposits if it still owes anyone a refund. Address
# and rpc default from frontend/.env.local, like `just deployer`. Set FORCE=1 to skip
# the confirmation prompt. Examples:
#   just destroy                           # use frontend/.env.local
#   just destroy 0xCONTRACT https://eth-rpc.polkadot.io/
destroy addr="" rpc="":
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cast >/dev/null || { echo "✖ cast not found — install Foundry: https://getfoundry.sh"; exit 1; }
    : "${PRIVATE_KEY:?✖ set PRIVATE_KEY (export it, or put it in a root .env)}"
    env=frontend/.env.local
    addr="{{addr}}"
    rpc="{{rpc}}"
    if [ -f "$env" ]; then
      [ -n "$addr" ] || addr=$(grep -E '^NEXT_PUBLIC_CONTRACT_ADDRESS=' "$env" | head -1 | cut -d= -f2-)
      [ -n "$rpc"  ] || rpc=$(grep -E '^NEXT_PUBLIC_RPC_URL=' "$env" | head -1 | cut -d= -f2-)
    fi
    rpc="${rpc:-{{paseo_rpc}}}"
    [ -n "$addr" ] || { echo "✖ no contract address — pass one (just destroy 0x… [rpc]) or run: just frontend-env <addr> <rpc>"; exit 1; }
    caller=$(cast wallet address --private-key "$PRIVATE_KEY" 2>/dev/null) || {
      echo "✖ invalid PRIVATE_KEY — expected a 0x-prefixed 32-byte secp256k1 key"; exit 1; }
    # Fail fast if this key isn't the deployer: destroy() would just revert with
    # NotOwner (and may leave a banned tx in the pool — see `just unban`).
    onchain=$(cast call "$addr" "deployer()(address)" --rpc-url "$rpc")
    if [ "$(printf '%s' "$caller" | tr 'A-F' 'a-f')" != "$(printf '%s' "$onchain" | tr 'A-F' 'a-f')" ]; then
      echo "✖ PRIVATE_KEY ($caller) is not the deployer ($onchain) — destroy() would revert with NotOwner"; exit 1
    fi
    echo "▸ about to destroy $addr on $rpc"
    echo "  caller: $caller (deployer ✓)"
    if [ "${FORCE:-0}" != "1" ]; then
      read -r -p "⚠ IRREVERSIBLE — this removes the contract and sweeps its balance. Type 'destroy' to confirm: " reply
      [ "$reply" = "destroy" ] || { echo "aborted"; exit 1; }
    fi
    out=$(cast send "$addr" "destroy()" --private-key "$PRIVATE_KEY" --rpc-url "$rpc" --json)
    echo "$out"
    echo "✔ destroy() sent — verify removal: just deployer $addr $rpc (should now fail to return)"

# Point the frontend at a deployed contract (rewrites frontend/.env.local). The
# chain identity (id/name/symbol/testnet) is derived from the rpc so the build
# self-labels correctly — mainnet builds show "Polkadot Hub", not "Paseo Hub".
frontend-env addr rpc=paseo_rpc:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{rpc}}" in
      *eth-rpc.polkadot.io*)        id=420420419; name="Polkadot Hub"; sym=DOT; testnet=false ;;
      *eth-rpc-kusama.polkadot.io*) id=420420418; name="Kusama Hub";   sym=KSM; testnet=false ;;
      *)                            id=420420417; name="Polkadot Hub TestNet"; sym=PAS; testnet=true ;;
    esac
    f=frontend/.env.local
    {
      echo "NEXT_PUBLIC_CONTRACT_ADDRESS={{addr}}"
      echo "NEXT_PUBLIC_RPC_URL={{rpc}}"
      echo "NEXT_PUBLIC_CHAIN_ID=$id"
      echo "NEXT_PUBLIC_CHAIN_NAME=$name"
      echo "NEXT_PUBLIC_CHAIN_SYMBOL=$sym"
      echo "NEXT_PUBLIC_CHAIN_TESTNET=$testnet"
    } > "$f"
    echo "✔ wrote $f ($name, chain $id)"

# Install frontend dependencies.
frontend-install:
    cd frontend && npm install

# Start the frontend dev server (http://localhost:8080).
dev:
    cd frontend && npm run dev -- -p 8080

# Run the frontend Vitest suite (unit + component tests). No wallet/node needed.
frontend-test:
    cd frontend && npm test

# Build the frontend into a static site at frontend/out/ (next export). The
# NEXT_PUBLIC_* env (contract address, rpc) is baked in at build time, so run
# `just frontend-env <addr>` first. Serve out/ from any static host — no Node.
frontend-build:
    cd frontend && npm run build

# Preview the static build locally at http://localhost:3000.
frontend-serve: frontend-build
    cd frontend && npx --yes serve@latest out -l 3000

# Serve the static build with Caddy (frontend/Caddyfile) at http://localhost:3000.
frontend-serve-caddy: frontend-build
    #!/usr/bin/env bash
    set -euo pipefail
    command -v caddy >/dev/null || { echo "✖ caddy not found — https://caddyserver.com/docs/install"; exit 1; }
    cd frontend && caddy run --config Caddyfile

# Deploy the static build to the server (chamber.tasty.limo) over ssh/rsync. Serves
# from /srv/chamber/out via Caddy's import layout (sites/chamber.caddy). NOTE: the
# NEXT_PUBLIC_* values are baked in at build time, so run `just frontend-env <addr> <rpc>`
# for the target network BEFORE deploying. Requires DNS A chamber.tasty.limo -> the host.
frontend-deploy host="server": frontend-build
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ ensuring web root on {{host}}"
    ssh {{host}} 'sudo mkdir -p /srv/chamber && sudo chown "$(id -un):$(id -gn)" /srv/chamber'
    echo "▸ syncing frontend/out/ → {{host}}:/srv/chamber/out/"
    rsync -az --delete -e ssh frontend/out/ {{host}}:/srv/chamber/out/
    echo "▸ installing site config → /etc/caddy/sites/chamber.caddy"
    rsync -az -e ssh frontend/deploy/chamber.caddy {{host}}:/tmp/chamber.caddy
    ssh {{host}} 'sudo install -m644 /tmp/chamber.caddy /etc/caddy/sites/chamber.caddy && rm -f /tmp/chamber.caddy'
    echo "▸ validating + reloading caddy"
    ssh {{host}} 'sudo caddy validate --config /etc/caddy/Caddyfile && sudo systemctl reload caddy'
    echo "✔ deployed — https://chamber.tasty.limo/"

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
account rpc=paseo_rpc:
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

# Point the frontend at a deployed contract (rewrites frontend/.env.local).
frontend-env addr rpc=paseo_rpc:
    #!/usr/bin/env bash
    set -euo pipefail
    f=frontend/.env.local
    { echo "NEXT_PUBLIC_CONTRACT_ADDRESS={{addr}}"; echo "NEXT_PUBLIC_RPC_URL={{rpc}}"; } > "$f"
    echo "✔ wrote $f"

# Install frontend dependencies.
frontend-install:
    cd frontend && npm install

# Start the frontend dev server (http://localhost:3000).
dev:
    cd frontend && npm run dev

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

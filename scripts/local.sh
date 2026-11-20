#!/usr/bin/env bash
# Usage: ./scripts/local.sh [up|down]
# Starts/stops local standalone Stellar quickstart container, funds deployer, deploys, and initialises contracts.
set -euo pipefail
cd "$(dirname "$0")/.."

CONTAINER_NAME="sorobanpool-quickstart"
NETWORK_PASSPHRASE="Standalone Network ; February 2017"
RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
HORIZON_URL="${HORIZON_URL:-http://localhost:8000}"
FRIENDBOT_URL="${FRIENDBOT_URL:-http://localhost:8000/friendbot}"
SOURCE="${SOURCE:-sorobanpool-deployer}"

up() {
  echo "==> Starting Stellar quickstart container..."
  if docker ps -a --format '{{.Names}}' | grep -Eq "^${CONTAINER_NAME}\$"; then
    docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
  fi

  docker run -d \
    --name "$CONTAINER_NAME" \
    -p 8000:8000 \
    stellar/quickstart:testing \
    --standalone \
    --enable-soroban-rpc

  echo "==> Waiting for standalone network RPC to become healthy..."
  for i in $(seq 1 60); do
    if curl -s -f -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
      "$RPC_URL" 2>/dev/null | grep -q '"status":"healthy"'; then
      echo "Stellar standalone node is ready."
      break
    fi
    sleep 1
    if [ "$i" -eq 60 ]; then
      echo "Timed out waiting for standalone network" >&2
      exit 1
    fi
  done

  echo "==> Configuring stellar-cli local network and deployer identity..."
  stellar network add \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    local --overwrite >/dev/null 2>&1 || true

  if ! stellar keys address "$SOURCE" >/dev/null 2>&1; then
    stellar keys generate "$SOURCE" --network local >/dev/null 2>&1 || true
  fi

  DEPLOYER_ADDR="$(stellar keys address "$SOURCE")"
  echo "==> Funding deployer account: $DEPLOYER_ADDR"
  curl -s -f "${FRIENDBOT_URL}?addr=${DEPLOYER_ADDR}" >/dev/null || true

  echo "==> Deploying contracts and running initialization..."
  NETWORK=local SOURCE="$SOURCE" RPC_URL="$RPC_URL" scripts/deploy.sh

  echo "==> Local environment ready! Deployed addresses written to deployments/local.json"
}

down() {
  echo "==> Tearing down Stellar quickstart container..."
  if docker ps -a --format '{{.Names}}' | grep -Eq "^${CONTAINER_NAME}\$"; then
    docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1
    echo "Removed container $CONTAINER_NAME."
  else
    echo "No running container named $CONTAINER_NAME found."
  fi
}

case "${1:-up}" in
  up)
    up
    ;;
  down)
    down
    ;;
  *)
    echo "Usage: $0 [up|down]" >&2
    exit 1
    ;;
esac

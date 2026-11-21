#!/usr/bin/env bash
set -euo pipefail

# Build packages for all seven contracts.
for c in config registry group_buy disputes reputation supplier_bond smart_wallet; do
    stellar contract bindings typescript \
        --wasm "target/wasm32v1-none/release/$c.wasm" \
        --output-dir "bindings/$c" --overwrite
done
python3 scripts/align_bindings_version.py
python3 scripts/export_artifacts.py

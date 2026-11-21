# Common developer recipes for SorobanPool contracts

default:
    @just --list

# Check code formatting and run clippy linter
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

# Build workspace and compile contract WASM binaries
build:
    cargo build --workspace
    stellar contract build

# Run all workspace unit and scenario tests
test:
    cargo test --workspace

# Generate and verify TypeScript bindings for all contracts
bindings:
    scripts/bindings.sh

# Export contract errors.json and events.json artifacts
artifacts:
    python3 scripts/export_artifacts.py

# Generate line coverage summary report
cov:
    cargo llvm-cov --workspace --summary-only

# Run full validation suite (lint, build, test, bindings, artifacts)
check: lint test bindings artifacts

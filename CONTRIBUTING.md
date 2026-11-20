# Contributing
- Branches: `feat/…`, `fix/…`, `chore/…`. Conventional Commits. Squash merges.
- Every PR must build, pass fmt/clippy/tests, update docs, and reference an issue.
- No `unwrap()`/`expect()` in contract code. Never commit secrets.
- Escrow, settlement, disputes and bond changes need two maintainer reviews.

## Common Tasks (`justfile`)

A `justfile` is provided for common developer workflows. Run `just --list` to view available targets:

| Command | Action | Wrapped Command |
|---|---|---|
| `just lint` | Code style & clippy checks | `cargo fmt --all -- --check` && `cargo clippy --workspace --all-targets -- -D warnings` |
| `just build` | Build contracts & workspace | `cargo build --workspace` && `stellar contract build` |
| `just test` | Run all workspace tests | `cargo test --workspace` |
| `just bindings` | Regenerate TypeScript bindings | `scripts/bindings.sh` |
| `just artifacts` | Export errors & events JSON | `python3 scripts/export_artifacts.py` |
| `just cov` | Run test line coverage | `cargo llvm-cov --workspace --summary-only` |
| `just check` | Full local CI check suite | `just lint test bindings artifacts` |

## Adding a new contract function

See **[docs/adding-a-function.md](docs/adding-a-function.md)** for the full step-by-step checklist: implementation → tests → WASM build → TypeScript bindings → artifact export → docs → downstream notifications.

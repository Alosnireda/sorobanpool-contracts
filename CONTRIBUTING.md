# Contributing
- Branches: `feat/…`, `fix/…`, `chore/…`. Conventional Commits. Squash merges.
- Every PR must build, pass fmt/clippy/tests, update docs, and reference an issue.
- No `unwrap()`/`expect()` in contract code. Never commit secrets.
- Escrow, settlement, disputes and bond changes need two maintainer reviews.

## Adding a new contract function

See **[docs/adding-a-function.md](docs/adding-a-function.md)** for the full step-by-step checklist: implementation → tests → WASM build → TypeScript bindings → artifact export → docs → downstream notifications.

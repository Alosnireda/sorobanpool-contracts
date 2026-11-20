# Adding a New Contract Function

This guide walks through every step required to ship a new public function — from the Rust implementation to updated bindings, artifacts, and downstream notifications. It uses a small hypothetical example: `get_pool_stats` on the `registry` contract.

---

## 0. Before you start

- Open (or find) a GitHub issue describing the function. All PRs must reference an issue.
- Create a branch: `feat/<contract>-<short-name>` (e.g. `feat/registry-get-pool-stats`).
- Confirm you can build cleanly: `bash scripts/build.sh`.

---

## 1. Implement the function

### 1a. Add the function to the contract

Open `contracts/<contract>/src/lib.rs`. Add your function inside the `#[contract]` impl block.

```rust
// contracts/registry/src/lib.rs

pub fn get_pool_stats(env: Env) -> PoolStats {
    // read storage, compute, return
    storage::read_pool_stats(&env)
}
```

Rules:
- No `unwrap()` or `expect()` in contract code.
- Use `Result<T, Error>` for fallible functions; return `Error` variants defined in the same file.
- Keep functions small — extract helpers into `crates/` when logic is reused across contracts.

### 1b. Add a new error variant (if needed)

Error enums are namespaced by contract (config 100s, registry 200s, group_buy 300s, disputes 400s, supplier_bond 500s, reputation 600s). Append to the existing `pub enum Error` block:

```rust
pub enum Error {
    // … existing variants …
    PoolStatsNotReady = 207,
}
```

Add a matching row to `docs/errors.md`:

```markdown
| 207 | registry | PoolStatsNotReady |
```

### 1c. Emit an event (if the function mutates state)

View functions typically don't emit events. If yours does, add the event name and fields to `docs/events.md` and update `contracts/<contract>/src/lib.rs` to publish it:

```rust
env.events().publish(("pool_stats_updated",), (old_total, new_total));
```

---

## 2. Write tests

Add unit tests in `contracts/<contract>/src/lib.rs` (inline `#[cfg(test)]` module) or in a dedicated file under `contracts/<contract>/src/tests/`:

```rust
#[test]
fn test_get_pool_stats_returns_zero_on_fresh_contract() {
    let env = Env::default();
    let contract_id = env.register_contract(None, RegistryContract);
    let client = RegistryContractClient::new(&env, &contract_id);
    let stats = client.get_pool_stats();
    assert_eq!(stats.total_members, 0);
}
```

Run the suite before moving on:

```bash
cargo test --workspace
```

---

## 3. Format and lint

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Fix every warning — the CI enforces a zero-warning policy.

---

## 4. Build the WASM

```bash
bash scripts/build.sh
# produces target/wasm32v1-none/release/<contract>.wasm
```

---

## 5. Update the TypeScript bindings

```bash
bash scripts/bindings.sh
# regenerates bindings/<contract>/ from the freshly built WASM
```

Review the diff under `bindings/<contract>/` to confirm your new function appears with the correct argument and return types. Commit the regenerated files — they are the source of truth for the frontend and backend SDK.

---

## 6. Regenerate the artifacts

```bash
python3 scripts/export_artifacts.py
# rewrites artifacts/errors.json and artifacts/events.json
```

Commit `artifacts/errors.json` and `artifacts/events.json`. The frontend error map and the backend indexer fixtures consume these files directly.

---

## 7. Update the docs

| File | What to add |
|------|-------------|
| `docs/errors.md` | New error rows (Step 1b) |
| `docs/events.md` | New event rows (Step 1c, if applicable) |
| `docs/architecture.md` | Update if the function changes a component boundary |
| `docs/state-machine.md` | Update if the function introduces a new state transition |
| Contract-specific doc under `docs/` | Add a brief description of the function, its arguments, and return value |

---

## 8. Downstream repos and notifications

Changes to the bindings or artifacts affect two downstream repos. After your PR is merged into `main`:

| Downstream repo | What to do |
|----------------|------------|
| **sorobanpool-frontend** | Open a follow-up issue (or PR) in that repo referencing this one. The bindings package must be bumped to pick up the new TypeScript types. Tag `@sorobanpool/frontend-team` in the issue body. |
| **sorobanpool-backend** | The backend indexer reads `artifacts/errors.json` and `artifacts/events.json`. Open a follow-up issue (or PR) referencing this one to update the error map and event handler. Tag `@sorobanpool/backend-team`. |

If your function changes on-chain storage layout in a backward-incompatible way, also notify the **deployments** maintainer and update the relevant file under `deployments/`.

Post a brief summary in the `#contracts` channel linking both follow-up issues.

---

## 9. Open the pull request

- Title: `feat(<contract>): add <function-name>` (Conventional Commits).
- Body: reference the issue (`Closes #<n>`), summarise the change, list the downstream follow-ups.
- Escrow, settlement, disputes, and bond changes require two maintainer reviews; all others require one.

---

## Checklist

```
[ ] Function implemented in contracts/<contract>/src/lib.rs
[ ] New Error variant added (if needed) + docs/errors.md updated
[ ] New event documented in docs/events.md (if applicable)
[ ] Unit tests added and passing (cargo test --workspace)
[ ] cargo fmt --all and cargo clippy pass
[ ] WASM rebuilt (bash scripts/build.sh)
[ ] TypeScript bindings regenerated (bash scripts/bindings.sh) and committed
[ ] Artifacts regenerated (python3 scripts/export_artifacts.py) and committed
[ ] docs/ updated (errors.md, events.md, architecture.md, state-machine.md as needed)
[ ] PR opened with issue reference and downstream follow-ups noted
[ ] Downstream issues/PRs opened in sorobanpool-frontend and sorobanpool-backend
[ ] #contracts channel notified with links to all follow-ups
```

---

## Friction log

If you followed this guide on a scratch branch and hit a snag, please open an issue (label: `documentation`) with what was unclear so we can improve it.

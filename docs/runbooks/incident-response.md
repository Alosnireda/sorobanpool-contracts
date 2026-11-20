# Incident Response Runbook

**Version:** 1.0 — initial (issue #121)
**Contracts touched:** `config` (`pause`, `unpause`, `set_params`, `upgrade`)
**Admin authority required:** multisig (see [threat model](../threat-model.md))

---

## Severity classification

| Severity | Definition | Maximum response time |
|---|---|---|
| **P0 — Critical** | Active fund loss or fund drain in progress | Pause within 10 minutes |
| **P1 — High** | Defect confirmed, funds not yet at risk but exploit is known | Pause within 1 hour |
| **P2 — Medium** | Defect confirmed, no immediate fund risk, no known exploit | Coordinate fix without pause |
| **P3 — Low** | UI/backend defect, contracts unaffected | Normal release cycle |

---

## 1. Decision tree — which scope to pause

The `pause(caller, scope)` function in `contracts/config/src/lib.rs` accepts one of four
scope symbols.  Pausing **never** blocks refunds, expiry, failure paths (`close`,
`fail_accept`, `fail_delivery`), dispute timeouts, or settlement — users can always exit.

```
Incident detected
        │
        ▼
Is ANY scope affected by the defect?
  No ──────────────────────────────────────────► Do NOT pause. Monitor and patch.
  Yes
        │
        ▼
Which contract function is implicated?
        │
        ├─ Pool creation (`group_buy.create`) ──────────────────► scope: "create"
        │
        ├─ Member commitment (`group_buy.commit`) ──────────────► scope: "commit"
        │
        ├─ Supplier/organizer acceptance (`group_buy.accept`) ──► scope: "accept"
        │
        └─ All of the above, or unknown blast radius ───────────► scope: "all"
```

> **Rule of thumb:** prefer the narrowest scope.  `scope="all"` is a last resort
> because it freezes pool creation for new members (existing members can still exit).

### Who can pause

- **Admin** (multisig): any scope
- **Guardian** (single key, hot): any scope — the guardian key exists for speed.
  It MUST be rotated if compromised.  See `config.set_guardian`.

---

## 2. Step-by-step: emergency pause

### Prerequisites

```bash
# Confirm the config contract address from the deployment manifest:
CONFIG_ID=$(jq -r '.contracts.config.id' deployments/testnet.json)   # or mainnet.json
NETWORK=testnet   # change to mainnet for production
SOURCE=sorobanpool-deployer   # or the multisig signing key name
```

### Step 1 — Verify the scope is not already paused

```bash
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- is_paused --scope create
# → true  (already paused)
# → false (proceed to pause)
```

### Step 2 — Pause

```bash
# Replace CALLER with the admin or guardian address.
CALLER=$(stellar keys address "$SOURCE")

stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- pause \
    --caller "$CALLER" \
    --scope create       # or commit | accept | all
```

Expected output: a transaction hash.  Confirm with:

```bash
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- is_paused --scope create
# → true
```

### Step 3 — Notify

Immediately post in the incident channel (Slack / Discord):

```
[P0 INCIDENT] sorobanpool-contracts paused (scope=<scope>)
Reason: <one-line description>
Affected: <what users cannot do>
Still available: refunds, expiry, failure paths, dispute timeouts, settlement
Point of contact: @<on-call handle>
Next update: <timestamp + 30 min>
```

### Step 4 — Monitor

Watch the `config` contract events for unexpected calls while paused:

```bash
stellar events --id "$CONFIG_ID" --network "$NETWORK" --start-ledger $(stellar ledger current --network "$NETWORK")
```

---

## 3. What stays available to users during a pause

| Action | Available during `scope=create` | `scope=commit` | `scope=accept` | `scope=all` |
|---|---|---|---|---|
| Create a new pool | ❌ | ✅ | ✅ | ❌ |
| Commit to a pool | ✅ | ❌ | ✅ | ❌ |
| Accept / confirm delivery | ✅ | ✅ | ❌ | ❌ |
| Withdraw from open pool | ✅ | ✅ | ✅ | ✅ |
| Close / fail / expiry paths | ✅ | ✅ | ✅ | ✅ |
| Claim refund | ✅ | ✅ | ✅ | ✅ |
| Open or settle a dispute | ✅ | ✅ | ✅ | ✅ |
| Dispute timeout / settlement | ✅ | ✅ | ✅ | ✅ |

> The contracts enforce this: `is_paused` is only checked at entry points for
> `create`, `commit`, and `accept` in `group_buy`.  Exit paths are permissionless.

---

## 4. Resuming after a pause

```bash
# Only admin can unpause.
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- unpause --scope create   # match the scope used to pause
```

Confirm and announce in the incident channel that normal operations have resumed.

---

## 5. Emergency parameter change (`set_params`)

Use this when a defect requires tuning a protocol parameter (e.g. fee, pool cap) without
upgrading.  `set_params` is admin-only and validates the new params before applying.

```bash
# Fetch current params for reference:
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- get_params

# Apply new params (all fields required; use get_params output as template):
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- set_params \
    --params '{
      "platform_fee_bp": 50,
      "max_pool_cap_usdc": 500000,
      "min_commit_usdc": 1000,
      "dispute_deposit_bp": 100,
      "dispute_deposit_min_usdc": 50,
      "early_release_weight": 70
    }'
```

The contract emits a `params/updated` event with the new `platform_fee_bp` on success.
Announce in the incident channel.

---

## 6. Emergency upgrade

> **Warning:** `upgrade` is the most powerful operation available.  It requires the
> admin multisig.  The guardian key **cannot** upgrade.

### Multisig requirements

The admin key MUST be a Stellar multisig account before mainnet (see threat model).
Recommended threshold: 2-of-3 signers.  Each signer executes the same `upgrade` call;
the network holds the transaction until the threshold is met.

### Step-by-step

**Before starting:**

- [ ] Reproduce the defect on testnet with a full fix.
- [ ] Obtain a second review from a contributor who was not the primary author.
- [ ] All existing tests pass with the patched code (`cargo test`).
- [ ] CI is green on the patch branch.
- [ ] Announce the upgrade intent in the incident channel with a 15-minute window
      for objections (waive only for P0).

**Step 1 — Build the patched WASM**

```bash
scripts/build.sh
```

**Step 2 — Upload the new WASM and record its hash**

```bash
NEW_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/config.wasm \
  --source "$SOURCE" \
  --network "$NETWORK")
echo "New WASM hash: $NEW_WASM_HASH"
```

**Step 3 — Verify the hash matches the local build**

```bash
LOCAL_HASH=$(shasum -a 256 target/wasm32v1-none/release/config.wasm | cut -d' ' -f1)
[ "$NEW_WASM_HASH" = "$LOCAL_HASH" ] && echo "Hash matches" || echo "MISMATCH — do not proceed"
```

**Step 4 — Execute the upgrade (requires multisig)**

```bash
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- upgrade --wasm_hash "$NEW_WASM_HASH"
```

The contract emits an `upgraded/config` event with the new WASM hash on success.

**Step 5 — Smoke test**

```bash
# Verify the contract responds after the upgrade.
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- get_params
```

**Step 6 — Unpause (if paused)**

```bash
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network "$NETWORK" -- unpause --scope all
```

**Step 7 — Post-incident record**

Update `deployments/<network>.json` with the new WASM hash:

```bash
scripts/export_artifacts.py   # or manually update the wasmHash field
```

Write a brief post-mortem in `docs/runbooks/post-mortems/<YYYY-MM-DD>-<slug>.md`.

---

## 7. Testnet dry-run record (issue #121)

The following commands were executed against Soroban testnet to verify the runbook.
Contract ID used: the `config` contract from the latest `deployments/testnet.json`.

```bash
# Environment
NETWORK=testnet
CONFIG_ID=$(jq -r '.contracts.config.id' deployments/testnet.json)
SOURCE=sorobanpool-deployer
CALLER=$(stellar keys address "$SOURCE")

# 1. Confirm not paused
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- is_paused --scope create
# → false ✓

# 2. Pause scope=create
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- pause --caller "$CALLER" --scope create
# → tx hash: <redacted testnet tx> ✓

# 3. Confirm paused
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- is_paused --scope create
# → true ✓

# 4. Confirm other scopes unaffected
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- is_paused --scope commit
# → false ✓

# 5. Unpause
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- unpause --scope create
# → tx hash: <redacted testnet tx> ✓

# 6. Confirm unpaused
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- is_paused --scope create
# → false ✓

# 7. get_params smoke test
stellar contract invoke --id "$CONFIG_ID" --source "$SOURCE" \
  --network testnet -- get_params
# → {"platform_fee_bp":50,...} ✓

# upgrade dry-run (scope: upload only, no invoke on shared testnet config)
scripts/build.sh
stellar contract upload \
  --wasm target/wasm32v1-none/release/config.wasm \
  --source "$SOURCE" --network testnet
# → wasm hash printed ✓; shasum verified to match local build ✓
```

All steps completed without error.

---

## 8. Related documents

- [Threat model](../threat-model.md) — admin/guardian trust model
- [State machine](../state-machine.md) — permissionless exit paths
- [Mainnet checklist](../mainnet-checklist.md) — multisig requirement (open item)
- [contracts/config/src/lib.rs](../../contracts/config/src/lib.rs) — `pause`, `unpause`, `set_params`, `upgrade` source

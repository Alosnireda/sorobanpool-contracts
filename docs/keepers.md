# Keeper guide

A *keeper* is any off-chain process (cron job, cloud function, bot) that calls permissionless contract entry-points to advance pool state. No privileged key is required — keepers are economically incentivised via gas rebates or separate reward tokens; anyone can run one.

## When to act

A pool needs keeper attention during two lifecycle phases:

| Phase | Entry-points | Trigger condition |
|---|---|---|
| Shortfall allocation | `allocate_shortfall(pool_id, max)` | Pool reached `FillFailed` state |
| Refund distribution | `push_refunds(pool_id, max)` | Pool reached `Cancelled`, `FillFailed`, or `Settled` state **and** `refunds_pending` returns a non-zero count |

## `refunds_pending(pool_id)` — the keeper's decision helper

```typescript
import { Client } from "@sorobanpool/group-buy-bindings";

const client = new Client({ contractId, networkPassphrase, rpcUrl });

const { result } = await client.refunds_pending({ pool_id });
const [remainingMembers, remainingAmount] = result;
```

The view returns `[remaining_members, remaining_amount]` atomically from on-chain state:

- **`remaining_members`** — how many members have not yet received their keeper-pushed refund (`listed_members - refund_cursor`). Equivalent to the number of additional `push_refunds` iterations needed when `max = 1`.
- **`remaining_amount`** — total USDC (in stroops) still owed across all un-paid members (`owed_total - refunds_paid`).

Both values are `0` when the pool is not in a final-refundable state, so it is safe to call this view on any pool without checking state first.

## Recommended batch loop

```typescript
const BATCH_SIZE = 50; // tune to stay within Soroban CPU budget

async function drainRefunds(pool_id: bigint) {
  while (true) {
    const { result: [remaining] } = await client.refunds_pending({ pool_id });
    if (remaining === 0) break;

    const tx = await client.push_refunds({ pool_id, max: BATCH_SIZE });
    await tx.signAndSend();

    console.log(`pushed batch; ${remaining} members remaining before this call`);
  }
}
```

## Budget guidance

Each `push_refunds` invocation transfers USDC to up to `max` members. The on-chain CPU budget allows roughly **50–80 members per transaction** at current Soroban limits. Set `max = 50` to stay comfortably within budget; adjust upward after profiling on testnet.

## Monitoring

A simple health check:

```typescript
const { result: [members, amount] } = await client.refunds_pending({ pool_id });
if (members > 0) {
  alert(`Pool ${pool_id}: ${members} members still awaiting refund (${amount} stroops)`);
}
```

Emit this metric from your keeper every run. When `members` reaches `0` the pool is fully drained and no further keeper action is needed.

## Related entry-points

| Function | Purpose |
|---|---|
| `push_refunds(pool_id, max)` | Pay up to `max` members from `refund_cursor` forward; returns number visited |
| `claim_refund(pool_id)` | Member self-service alternative; callable at any time after pool goes refundable |
| `sweep_dust(pool_id)` | Reclaim any rounding dust after all members are paid; safe to call when `refunds_pending` returns `[0, _]` |

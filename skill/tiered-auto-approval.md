# Tiered Auto-Approval Model

Cerberus defines a 3-tier approval model for agent spending based on the
requested amount relative to the on-chain `SpendingLimit`.

## Tiers

| Tier | Condition | Who approves | On-chain enforcement |
|------|-----------|--------------|---------------------|
| 1 | `amount <= remaining_this_period` | No one — auto-approved | Yes — Squads program enforces |
| 2 | `remaining < amount <= 5 * limit` | Extra off-chain verification | No — application layer only |
| 3 | `amount > 5 * limit` | Human multisig vote | Yes — Squads proposal flow |

## Tier 1 — On-Chain Auto-Approval

The transaction is within the `SpendingLimit.remaining_amount` for this period.
The agent signs alone; no co-signer, no webhook, no human in the loop.

```
Agent wallet → spending_limit_use instruction → Squads validates → TX succeeds
```

The Squads program decrements `remaining_amount` atomically. No race condition
is possible. See `examples/use_spending_limit.rs` for the full implementation.

## Tier 2 — Extra Verification (Application Layer)

The transaction exceeds the remaining period budget but is less than 5× the
total limit. This tier is for legitimate one-off purchases that an agent might
need but that should not auto-approve without additional context.

**Tier 2 is not enforced on-chain.** It is a pattern for application code
layered on top of the on-chain limit. Possible implementations:

- TOTP / hardware key confirmation from the wallet owner
- Webhook with a 60-second approval window
- Rate-limit check: has the agent triggered N Tier 2 requests today?
- Signed payload verification from a trusted origin

After secondary verification passes, the governing_authority can submit a
Squads vault transaction proposal to move funds that exceed the limit.

## Tier 3 — Human Multisig Vote

The transaction is so large it requires explicit governance. This catches
wallet-draining attempts or agents that have been manipulated via prompt
injection into requesting outsized transfers.

Flow:
1. Create a Squads proposal (`vault_transaction_create`).
2. Notify all multisig members (governing_authority).
3. Wait for threshold signatures (`vault_transaction_vote`).
4. Execute after any time-lock expires (`vault_transaction_execute`).

## Classifying Requests in Code

```rust
fn classify(amount: u64, remaining: u64, limit: u64) -> ApprovalTier {
    if amount <= remaining {
        ApprovalTier::Tier1AutoApproved
    } else if amount <= limit.saturating_mul(5) {
        ApprovalTier::Tier2ExtraVerification {
            shortfall: amount.saturating_sub(remaining),
        }
    } else {
        ApprovalTier::Tier3HumanApproval {
            excess_multiplier: amount.saturating_div(limit.max(1)),
        }
    }
}
```

Full working example in `examples/use_spending_limit.rs`.

## The Approval Gap Problem

Without Tier 2, agents face a binary choice: auto-approve (unlimited by the
on-chain budget) or hard-reject everything over the limit. An agent that runs
out of Tier 1 budget mid-task either stalls or has no fallback path.

Tier 2 fills this gap: it gives the agent a legitimate escalation path for
amounts that are "big but not suspicious", while keeping Tier 3 reserved for
amounts that should genuinely alarm a human.

## Why These Thresholds?

- **5× multiplier** is a practical default. A 0.01 SOL/day agent should flag
  anything above 0.05 SOL as requiring human review.
- **Only Tier 1 is enforced on-chain.** Tiers 2 and 3 are application-level
  guardrails you layer on top.
- Adjust multipliers in your own `classify` implementation. The 5× is
  a starting point, not a protocol constraint.

## Period Reset Behavior

`SpendingLimit` resets `remaining_amount` to `amount` at the start of each new
period. The reset is lazy: it happens on the next `spending_limit_use`
instruction after the period boundary, not at a scheduled time.

`last_reset` stores the unix timestamp of the last reset. To estimate when
the budget refills:

```rust
// period_seconds depends on SpendingPeriod:
// Day = 86_400, Week = 604_800, Month = 2_592_000
let seconds_until_reset = period_seconds - (now_unix - spending_limit.last_reset);
```

Deserialize `SpendingLimit` directly from RPC data to read `remaining_amount`
and `last_reset` in production.

# Partial Setup Failure Recovery

## Why Partial Failures Happen

Cerberus setup is four sequential on-chain transactions:

```
TX1  multisig_create_v2           → creates Multisig PDA (config_authority = payer)
TX2  multisig_add_spending_limit  → creates SpendingLimit PDA
TX3  multisig_change_threshold    → sets threshold to 1
TX4  multisig_set_config_authority → config_authority → Pubkey::default() (LOCK)
```

A failure between any two steps leaves the account partially set up:
- The Multisig PDA exists but may not be locked.
- The SpendingLimit PDA may or may not exist.
- The payer's key still has `config_authority` and can modify the multisig.

**This is a dangerous state.** An agent operating against an unlocked multisig
has no on-chain spending enforcement. Do not fund the vault until TX4 confirms.

Common causes of partial failure:
- RPC timeout after TX1–TX3 confirm but before TX4 lands
- Insufficient funds for rent on TX2 (payer balance estimation was off)
- Network partition between confirmations
- Application crash / process kill between steps

## Detecting Partial Setup

Read the `LockState` from the multisig PDA:

```rust
use cerberus_skill::lock::get_lock_state;

let state = get_lock_state(&rpc, &multisig_pda).await?;
println!("config_authority: {}", state.config_authority);
println!("threshold:        {}", state.threshold);

// Fully locked = config_authority is all-zeros AND threshold is correct
let is_locked = state.is_fully_locked(1);
```

If `config_authority != Pubkey::default()`, TX4 did not complete.
If `threshold != expected`, TX3 did not complete.

## Recovery Decision Tree

```
get_lock_state(multisig_pda)
│
├── is_fully_locked(expected_threshold) == true
│   └── No action needed. Setup is complete.
│
├── config_authority != Pubkey::default()
│   ├── threshold != expected_threshold
│   │   └── recover_partial_setup sends TX3 + TX4
│   └── threshold == expected_threshold
│       └── recover_partial_setup sends TX4 only
│
└── AccountNotFound
    └── Setup never started, or TX1 itself failed. Run full setup.
```

## Recovering with recover_partial_setup

```rust
use cerberus_skill::recover::recover_partial_setup;

// Idempotent: safe to call even if already fully locked.
// Sends only the missing transactions (TX3 and/or TX4).
// Returns Ok(()) if the multisig is fully locked after recovery.
recover_partial_setup(&rpc, &payer, &multisig_pda, 1).await?;
```

The `payer` must be the same keypair that was used as `config_authority` during
the original TX1. Only that key can sign TX3/TX4.

See `examples/finalize_lock.rs` for an interactive recovery tool, and
`examples/recover_orphaned_account.rs` for batch recovery from a file of PDAs.

## Preventing Partial Failures

1. **Pre-flight balance check** — Cerberus checks payer balance before TX1 and
   returns `CerberusError::InsufficientFunds` if it cannot cover all four
   transactions including rent.

2. **Persist the multisig PDA before funding the vault** — store
   `SpendingAccount.multisig_pda` in your database as soon as
   `create_governed_spending_account` returns. Even if the subsequent
   `assert_fully_locked` call fails, you know where the partial state is.

3. **Use `CommitmentConfig::confirmed`** — the default RPC commitment.
   Waiting for confirmed (not just processed) before treating a TX as done
   reduces the chance of a reorg invalidating the previous step.

4. **Do not re-run `create_governed_spending_account` to recover** — calling
   it again with a new `create_key` creates a second multisig, not a recovery.
   Use `recover_partial_setup` with the original `multisig_pda`.

## What Cannot Be Recovered

If TX2 (`multisig_add_spending_limit`) failed but TX1 succeeded, the multisig
exists but has no spending limit. `recover_partial_setup` only handles TX3/TX4.
To attach a spending limit to an existing multisig, the `config_authority` must
submit TX2 directly (or via `spending_account::add_spending_limit`). This is a
valid operation as long as `config_authority != Pubkey::default()`.

If TX4 succeeded but you cannot confirm it (e.g. RPC returned timeout before the
response arrived), `recover_partial_setup` is still safe — it reads the chain
state first and returns `Ok(())` immediately if already locked.

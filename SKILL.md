---
name: cerberus-skill
version: "0.1.0"
language: rust
tags: [solana, squads, spending-limits, ai-agents, x402, multisig, usdc, spl-token, stablecoin]
description: >
  On-chain governed spending limits for AI agent wallets on Solana — especially
  for USDC and other SPL token or native SOL spending where database-stored
  limits are not trustworthy enough (prompt injection, compromised keys, race
  conditions). Enforced by Squads Protocol v4 at the validator level.
entrypoint: src/lib.rs
examples: examples/
references: references/
license: MIT
---

# cerberus-skill

> Machine-readable entry point for AI coding agents.
> Human developers: see [README.md](README.md).

## Purpose

Give an AI agent wallet a mathematically enforced on-chain spending limit that
cannot be bypassed by prompt injection, a compromised API key, or a bug in
application code. Uses Squads Protocol v4. Enforced at the Solana validator
level, not in a database.

## When to Use This Skill

- You need to constrain how much SOL, USDC, or any SPL token an AI agent wallet
  can spend per day, week, or month.
- You need the limit to survive key compromise, prompt injection, and application
  bugs — database-stored stablecoin limits are a promise, not a law.
- You are building an agent that handles USDC payments, SOL transfers, or any
  SPL token and cannot afford a backend bug or injection attack to drain the wallet.
- You need a recovery path if setup is interrupted mid-flight.
- You need to verify on-chain that a previously created limit has not been tampered
  with.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- Funded Solana wallet (payer) with >= 0.015 SOL on devnet or >= 0.015 SOL
  equivalent on mainnet
- RPC endpoint (`https://api.devnet.solana.com` or a paid provider)
- The agent wallet pubkey you want to constrain

## Add as Dependency

```toml
# Cargo.toml
[dependencies]
cerberus-skill = "0.1"
```

## Setup (TX1–TX4)

```rust
use cerberus_skill::{
    create_governed_spending_account,
    spending_account::{SpendingTierConfig, SpendingPeriod},
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());

// payer:               funds setup, becomes config_authority temporarily
// agent_wallet:        the AI agent's signing key — constrained by the limit
// governing_authority: human key that can modify the multisig via governance vote
let account = create_governed_spending_account(
    &rpc,
    &payer,
    agent_wallet,
    governing_authority,
    SpendingTierConfig {
        max_auto_approve_lamports: 10_000_000, // 0.01 SOL per period
        period: SpendingPeriod::Day,
        mint: Pubkey::default(),               // Pubkey::default() = native SOL
    },
)
.await?;

// CRITICAL: persist these before funding the vault
// account.multisig_pda      — governance account
// account.vault_pda         — the vault the agent spends from
// account.spending_limit_pda — the on-chain enforcement account
```

`create_governed_spending_account` runs TX1–TX4 atomically in sequence:

1. `multisig_create_v2` — creates Multisig PDA (`config_authority = payer`)
2. `multisig_add_spending_limit` — attaches `SpendingLimit` PDA
3. `multisig_change_threshold` — confirms threshold
4. `multisig_set_config_authority` — sets `config_authority = Pubkey::default()` (locked)

After TX4, **no single key can modify the multisig**. Changes require a
`governing_authority` governance vote.

## Verify the Lock

```rust
use cerberus_skill::verify::assert_fully_locked;

// Returns Err(CerberusError::LockVerificationFailed) if anything is wrong.
assert_fully_locked(&rpc, &account.multisig_pda, 1).await?;
```

Also check the lock state manually:

```rust
use cerberus_skill::lock::get_lock_state;

let state = get_lock_state(&rpc, &account.multisig_pda).await?;
assert_eq!(state.config_authority, Pubkey::default()); // locked
assert_eq!(state.threshold, 1);
```

## Fund the Vault and Spend

```rust
use squads_multisig::{
    client::{spending_limit_use, SpendingLimitUseAccounts},
    squads_multisig_program as program,
    squads_multisig_program::instructions::SpendingLimitUseArgs,
};
use squads_multisig::pda::get_vault_pda;

let (vault_pda, _) = get_vault_pda(&account.multisig_pda, 0, Some(&program::ID));

// Fund vault first (system transfer from payer to vault_pda), then:
let ix = spending_limit_use(
    SpendingLimitUseAccounts {
        multisig: account.multisig_pda,
        member: agent_wallet,                         // agent signs alone
        spending_limit: account.spending_limit_pda,
        vault: vault_pda,
        destination: recipient_pubkey,
        system_program: Some(system_program::id()),   // SOL transfer
        mint: None,
        vault_token_account: None,
        destination_token_account: None,
        token_program: None,
    },
    SpendingLimitUseArgs {
        amount: 1_000_000,  // must be <= SpendingLimit.remaining_amount
        decimals: 9,
        memo: None,
    },
    Some(program::ID),
);
// Sign with agent only — governing_authority is NOT required for Tier 1 spends.
```

If `amount > remaining_amount`, the Squads program rejects the transaction
on-chain with `SpendingLimitExceeded` (error `0x1790`). No application check
needed — the chain enforces it.

## Recovery

If setup was interrupted after TX1 but before TX4:

```rust
use cerberus_skill::recover::recover_partial_setup;

// Idempotent — safe to call on already-locked multisigs (returns Ok immediately).
// payer must be the original config_authority key used in TX1.
recover_partial_setup(&rpc, &payer, &multisig_pda, 1).await?;
```

See `examples/finalize_lock.rs` (single PDA) and
`examples/recover_orphaned_account.rs` (batch from file).

## Public API

### `spending_account` module

```rust
// Creates the four-transaction Squads v4 setup.
pub async fn create_governed_spending_account(
    rpc: &RpcClient,
    payer: &Keypair,
    agent_wallet: Pubkey,
    governing_authority: Pubkey,
    tier: SpendingTierConfig,
) -> Result<SpendingAccount, CerberusError>

pub struct SpendingAccount {
    pub multisig_pda: Pubkey,
    pub vault_pda: Pubkey,
    pub spending_limit_pda: Pubkey,
}

pub struct SpendingTierConfig {
    pub max_auto_approve_lamports: u64,
    pub period: SpendingPeriod,
    pub mint: Pubkey,  // Pubkey::default() = native SOL
}

pub enum SpendingPeriod { OneTime, Day, Week, Month }
```

### `lock` module

```rust
pub async fn get_lock_state(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
) -> Result<LockState, CerberusError>

pub struct LockState {
    pub config_authority: Pubkey, // Pubkey::default() = locked
    pub threshold: u16,
}

impl LockState {
    pub fn is_fully_locked(&self, expected_threshold: u16) -> bool
}
```

### `verify` module

```rust
pub async fn assert_fully_locked(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    expected_threshold: u16,
) -> Result<(), CerberusError>
```

### `recover` module

```rust
pub async fn recover_partial_setup(
    rpc: &RpcClient,
    payer: &Keypair,
    multisig_pda: &Pubkey,
    expected_threshold: u16,
) -> Result<(), CerberusError>
```

### `error` module

```rust
pub enum CerberusError {
    RpcError(Box<ClientError>),
    LockVerificationFailed { field, expected, actual },
    PartialSetupDetected { multisig_pda, stage },
    InsufficientFunds { available_lamports, required_lamports },
    InvalidSpendingLimitAmount { reason },
    DeserializationError { address, message },
    SimulationFailed { logs },
}
```

## Approval Tiers

| Tier | Amount | Approver | On-chain? |
|------|--------|----------|-----------|
| 1 | <= remaining limit | None (auto) | Yes |
| 2 | <= 5x limit | Extra off-chain verification | No |
| 3 | > 5x limit | Human multisig vote | Yes |

See `references/tiered-auto-approval.md` and `examples/use_spending_limit.rs`.

## Critical Notes for Agents

- `Pubkey::default()` is native SOL in `mint`, not "no mint configured".
  Pass the correct SPL token mint for token-denominated limits.
- `config_authority = Pubkey::default()` means **locked** (disabled). This is
  the desired state after TX4. Do not confuse with `None` — the field is `Pubkey`.
- `recover_partial_setup` requires the same `payer` keypair used during TX1.
  The payer is the only key with `config_authority` until TX4 locks it.
- Never fund the vault until `assert_fully_locked` returns `Ok`. An unlocked
  vault has no on-chain spending enforcement.
- `SpendingPeriod::Month` maps to approximately 30 days (2,592,000 seconds)
  on-chain. There is no calendar-month boundary — it is a fixed duration.

## Files

```
src/spending_account.rs   create_governed_spending_account, SpendingTierConfig, SpendingPeriod
src/lock.rs               get_lock_state, LockState
src/verify.rs             assert_fully_locked
src/recover.rs            recover_partial_setup
src/error.rs              CerberusError, SetupStage

examples/create_agent_spending_account.rs   CLI: create a governed spending account
examples/finalize_lock.rs                   CLI: recover an interrupted setup
examples/use_spending_limit.rs              CLI: spend from vault with agent signature
examples/recover_orphaned_account.rs        CLI: batch recovery from a file of PDAs

references/why-database-limits-fail.md      Why DB limits are insufficient
references/squads-bootstrap-pattern.md      TX1-TX4 architecture + the 3 named bugs
references/partial-failure-recovery.md      Recovery decision tree
references/tiered-auto-approval.md          3-tier approval model
references/verification-checklist.md        Security audit checklist
references/common-errors.md                 Error reference + 3 named bugs
```

## Proof of Concept (devnet)

This multisig was created by running the TX1–TX4 bootstrap during Metera
development:

```
JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr
```

https://explorer.solana.com/address/JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr?cluster=devnet

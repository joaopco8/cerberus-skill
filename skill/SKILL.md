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
license: MIT
---

# cerberus-skill

> Machine-readable entry point for AI coding agents.
> Human developers: see [README.md](../README.md).

Give an AI agent wallet a mathematically enforced on-chain spending limit that
cannot be bypassed by prompt injection, a compromised API key, or a bug in
application code. Uses Squads Protocol v4. Enforced at the Solana validator
level, not in a database.

## When to Use This Skill

- Constrain how much SOL, USDC, or SPL token an AI agent wallet can spend per period
- Limit must survive key compromise, prompt injection, and application bugs
- Agent handles USDC payments, SOL transfers, or SPL tokens and cannot afford a drain
- Need a recovery path if setup is interrupted mid-flight
- Need to verify on-chain that a limit has not been tampered with

## Quick Start

```toml
# Cargo.toml — not yet on crates.io, use git reference
[dependencies]
cerberus-skill = { git = "https://github.com/joaopco8/cerberus-skill" }
```

```rust
use cerberus_skill::{
    create_governed_spending_account,
    spending_account::{SpendingTierConfig, SpendingPeriod},
};

let account = create_governed_spending_account(
    &rpc,
    &payer,
    agent_wallet,       // constrained AI key
    governing_authority, // human key for governance changes
    SpendingTierConfig {
        max_auto_approve_lamports: 10_000_000, // 0.01 SOL/day
        period: SpendingPeriod::Day,
        mint: Pubkey::default(),               // native SOL
    },
).await?;
// account.multisig_pda, .vault_pda, .spending_limit_pda — persist these
```

After setup: **no single key can modify the multisig**. Only a `governing_authority`
governance vote can change limits. This is enforced at the validator level.

## Live Security Demo

Real devnet run. Attacker has the agent private key only. All 3 blocked:

```
[1/3] overspend
      On-chain error: custom program error: 0x178a (SpendingLimitExceeded)

[2/3] config-escalation
      On-chain error: custom program error: 0x1774 (Unauthorized)

[3/3] vault-drain
      On-chain error: missing required signature for instruction

RESULT: 3/3 attack vectors blocked
```

Verify: https://explorer.solana.com/address/EU4sHFG4VwCY6CrmHF6A7VEvLU1HUWxPMJT3TjWkeZ81?cluster=devnet

## Reference Files (Load Only What You Need)

| File | Load when |
|------|-----------|
| [why-database-limits-fail.md](why-database-limits-fail.md) | explaining why DB limits are insufficient |
| [squads-bootstrap-pattern.md](squads-bootstrap-pattern.md) | implementing TX1–TX4 setup, understanding 3 named bugs |
| [partial-failure-recovery.md](partial-failure-recovery.md) | debugging a stuck/orphaned multisig |
| [tiered-auto-approval.md](tiered-auto-approval.md) | designing approval tiers above the auto-approve limit |
| [verification-checklist.md](verification-checklist.md) | auditing an existing integration |
| [common-errors.md](common-errors.md) | hitting a specific error code |

## Critical Notes

- `Pubkey::default()` in `mint` = native SOL. Pass the SPL token mint for token limits.
- `config_authority = Pubkey::default()` = **locked** (TX4 target state). Do not confuse with `None`.
- `recover_partial_setup` requires the same `payer` used in TX1.
- Never fund the vault until `assert_fully_locked` returns `Ok`.

## Proof of Concept (devnet)

```
JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr
```

https://explorer.solana.com/address/JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr?cluster=devnet

---

*Extracted from [Metera](https://metera.xyz) — a production x402 billing platform for AI agents on Solana.*

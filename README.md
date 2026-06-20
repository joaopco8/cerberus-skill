# cerberus-skill

On-chain governed spending limits for AI agent wallets on Solana, built on
[Squads Protocol v4](https://squads.so).

---

Database-stored spending limits are a promise, not a law. Any bug, prompt
injection, or compromised key can rewrite them. Cerberus replaces them with
cryptographically-enforced, on-chain rules stored in a Squads v4
`SpendingLimit` PDA. Once set, no agent action, no code bug, and no stolen
API key can bypass them — only a multisig governance vote can change them.

Cost: **~0.011 SOL per setup** (~$0.0015 at current prices). One-time cost
for the lifetime of the wallet.

---

## Quick Start

```sh
# Add to your project
cargo add cerberus-skill

# Or clone for examples
git clone https://github.com/joaopco8/cerberus-skill
cd cerberus-skill
cargo run --example create_agent_spending_account
```

---

## What It Does

```text
AI Agent Wallet
      │
      ▼
SpendingLimit PDA (Squads v4)
  ├── amount:   hard lamport cap per period  ← enforced by Solana runtime
  ├── period:   daily / weekly / monthly
  └── members:  who can authorize changes
        │
        ▼
  Multisig PDA ◄── human approvers (k-of-n)
```

The Solana runtime rejects any transaction that exceeds the `SpendingLimit`.
No application code runs. No database is consulted. The rule is in the chain.

---

## 3-Tier Governance Model

| Tier | Amount | Enforcement |
|------|--------|-------------|
| 1 | ≤ period limit | Auto-approved on-chain by Squads runtime |
| 2 | 1×–5× limit | Application-level extra verification (TOTP, webhook, etc.) |
| 3 | > 5× limit | Human multisig vote via Squads governance |

Tiers 2 and 3 are patterns you implement in your application layer, on top
of the on-chain Tier 1 guarantee. See `references/tiered-auto-approval.md`.

---

## Usage

```rust
use cerberus_skill::spending_account::{create_agent_spending_account, SpendingAccountConfig};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

let client = RpcClient::new("https://api.devnet.solana.com".to_string());
let payer = Keypair::new(); // your funded wallet

let info = create_agent_spending_account(
    &client,
    &payer,
    SpendingAccountConfig {
        agent_wallet: agent_keypair.pubkey(),
        approvers: vec![human_wallet.pubkey()],
        threshold: 1,
        limit_lamports: 10_000_000,  // 0.01 SOL per day
        period_seconds: 86_400,
        mint: Pubkey::default(),     // native SOL
        vault_index: 0,
    },
)?;

// Store info.create_key — needed for recovery if setup is interrupted.
println!("multisig: {}", info.multisig_pda);
```

For AI agent usage, see [SKILL.md](SKILL.md).

---

## Proof of Concept

This multisig was created during Cerberus development and is live on devnet:

**[JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr](https://explorer.solana.com/address/JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr?cluster=devnet)**

---

## Cost Breakdown

| Item | Lamports | SOL |
|------|----------|-----|
| Multisig PDA rent | ~7,000,000 | ~0.007 |
| SpendingLimit PDA rent | ~3,500,000 | ~0.0035 |
| Transaction fees (×2) | ~10,000 | ~0.00001 |
| **Total** | **~10,510,000** | **~0.0105** |

Rent is recoverable if the account is ever closed.

---

## API Reference

| Module | Key Function |
|--------|-------------|
| `spending_account` | `create_agent_spending_account` — full two-step setup |
| `lock` | `finalize_lock` — verify on-chain state after setup |
| `lock` | `remaining_budget` — pre-check budget before a transaction |
| `verify` | `verify_spending_limit` — full verification report |
| `recover` | `detect_partial_setup` — probe for interrupted setup |
| `recover` | `resume_setup` — continue from where setup failed |

Full docs: `cargo doc --open`

---

## Reference Documents

- [`references/why-database-limits-fail.md`](references/why-database-limits-fail.md) — attack vectors this solves
- [`references/squads-bootstrap-pattern.md`](references/squads-bootstrap-pattern.md) — raw instruction layout
- [`references/tiered-auto-approval.md`](references/tiered-auto-approval.md) — 3-tier governance pattern
- [`references/partial-failure-recovery.md`](references/partial-failure-recovery.md) — recovery decision tree
- [`references/verification-checklist.md`](references/verification-checklist.md) — on-chain verification checks
- [`references/common-errors.md`](references/common-errors.md) — error codes and fixes

---

## Attribution

Extracted from [Metera](https://metera.xyz), a production x402 billing
platform for AI agents on Solana. Metera uses this pattern to enforce
spending limits for every agent wallet it manages.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT — see [LICENSE](LICENSE).

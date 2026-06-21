---
description: "Audit a Cerberus multisig PDA for correct lock state and spending limit configuration"
---

You are auditing a Cerberus-managed Squads v4 multisig. Check that TX1–TX4 completed correctly and the account is fully locked.

## Usage

```
/cerberus-audit <MULTISIG_PDA>
```

## Step 1: Fetch Lock State

```rust
use cerberus_skill::lock::get_lock_state;

let state = get_lock_state(&rpc, &multisig_pda).await?;
println!("config_authority: {}", state.config_authority);
println!("threshold:        {}", state.threshold);
```

**Expected output for a fully locked account:**
- `config_authority` = `11111111111111111111111111111111` (Pubkey::default)
- `threshold` = 1 (or the expected value for this multisig)

If `config_authority` is any other pubkey, **TX4 did not complete** — the multisig is not locked and spending limits are not enforced.

## Step 2: Assert Fully Locked

```rust
use cerberus_skill::verify::assert_fully_locked;

match assert_fully_locked(&rpc, &multisig_pda, 1).await {
    Ok(()) => println!("✓ Fully locked — spending limits are enforced on-chain"),
    Err(e) => println!("✗ Lock verification failed: {e}"),
}
```

## Step 3: Check Spending Limit PDA

Fetch the SpendingLimit PDA account and verify:
- `amount` matches the expected cap
- `period` matches the configured period
- `mint` = `Pubkey::default()` for native SOL (or the expected SPL mint)
- `remaining_amount` resets each period

## Step 4: Run Attack Simulation (Optional, Requires CLI)

```sh
cargo run --bin cerberus -- simulate-attack <MULTISIG_PDA> \
  --agent-keypair <path/to/agent.json> \
  --spending-limit-pda <SPENDING_LIMIT_PDA>
```

Expected: `3/3 attack vectors blocked`

## Audit Report Template

```
Multisig PDA:       <address>
config_authority:   <Pubkey::default = locked | other = INCOMPLETE LOCK>
threshold:          <n>
SpendingLimit PDA:  <address>
  amount:           <lamports> (<SOL> SOL)
  period:           <Day|Week|Month|OneTime>
  mint:             <Pubkey::default = SOL | other = SPL token>
  remaining:        <lamports> this period

Lock status:        ✓ LOCKED | ✗ INCOMPLETE
```

## Common Findings

| Finding | Cause | Fix |
|---------|-------|-----|
| `config_authority ≠ default` | TX4 incomplete | Run `recover_partial_setup` |
| `threshold = 0` | TX3 incomplete | Run `recover_partial_setup` |
| `remaining = 0` | Period not reset yet | Wait for period boundary |
| Vault balance = 0 | Not funded | Transfer SOL/tokens to vault_pda |

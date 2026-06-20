# On-Chain Verification Checklist

Run this checklist after setup and periodically in production to confirm the
spending limit has not been tampered with.

## Squads v4 Program ID

All accounts must be owned by:

```
SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf
```

## Quick Verification (use cerberus-skill)

```rust
use cerberus_skill::verify::assert_fully_locked;

// Returns Err(CerberusError::LockVerificationFailed { .. }) if any check fails.
assert_fully_locked(&rpc, &multisig_pda, 1).await?;
```

## Manual Checklist

### Multisig PDA

- [ ] Account exists (`get_account` returns `Ok`)
- [ ] `account.owner == "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"`
- [ ] `multisig.config_authority == Pubkey::default()` (all-zeros = locked; **not** `None`)
- [ ] `multisig.threshold == 1` (or your configured value)
- [ ] `multisig.members.len() >= 1`
- [ ] `multisig.members` contains `governing_authority` with `Vote` permission

### SpendingLimit PDA

- [ ] Account exists (`get_account` returns `Ok`)
- [ ] `account.owner == "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"`
- [ ] `spending_limit.multisig == your_multisig_pda`
- [ ] `spending_limit.amount == your_configured_lamports`
- [ ] `spending_limit.period == Period::Day` (or your configured period)
- [ ] `spending_limit.mint == Pubkey::default()` (for native SOL)
- [ ] `spending_limit.members` contains your `agent_wallet`
- [ ] `spending_limit.remaining_amount <= spending_limit.amount`

## Deserializing Accounts

Squads v4 accounts use Anchor's 8-byte discriminator prefix:

```rust
use squads_multisig::{
    anchor_lang::AccountDeserialize,
    state::{Multisig, SpendingLimit},
};

let account = rpc.get_account(&multisig_pda).await?;
let multisig = Multisig::try_deserialize(&mut account.data.as_slice())
    .map_err(|e| CerberusError::DeserializationError {
        address: multisig_pda.to_string(),
        message: e.to_string(),
    })?;

// Check the lock sentinel:
assert_eq!(multisig.config_authority, Pubkey::default());
```

`try_deserialize` handles the 8-byte discriminator check automatically.
If it fails, the account is not a valid Squads Multisig (wrong program,
wrong version, or corrupted data).

## The Lock Sentinel

`config_authority` is `Pubkey` (not `Option<Pubkey>`). The disabled state is
represented by `Pubkey::default()` — 32 zero bytes, the System Program address
in base58: `11111111111111111111111111111111`.

Do **not** check for `None` — there is no `None`. Check for `== Pubkey::default()`.

```rust
// Correct:
if multisig.config_authority == Pubkey::default() {
    // locked
}

// Wrong — does not compile and would not be semantically correct:
// if multisig.config_authority.is_none() { ... }
```

## Recommended Verification Frequency

| Context | Frequency |
|---------|-----------|
| Immediately after setup | Once — in `assert_fully_locked` call |
| Agent startup | Every run |
| Long-running agent session | Every 24 hours |
| Before any high-value transaction | Every time |
| After any governance proposal executes | Immediately |

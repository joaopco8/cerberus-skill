# Common Errors and Fixes

## The Three Named Bugs

These bugs existed in the TypeScript proof-of-concept that Cerberus was ported
from. They are documented here as searchable patterns so future implementors do
not repeat them.

---

### Bug #1 — Treasury Mismatch (`InvalidTreasury`)

**Symptom:** TX1 (`multisig_create_v2`) fails with `InvalidTreasury` or an
account ownership error on the treasury account meta.

**Cause:** The caller passed the payer's pubkey (or a hardcoded address) as the
treasury account. The Squads program validates that the treasury matches the
address stored in the on-chain `ProgramConfig` PDA.

**Fix:** Fetch the treasury from the `ProgramConfig` account before building TX1:

```rust
use squads_multisig::pda::get_program_config_pda;
use squads_multisig::squads_multisig_program as program;
use squads_multisig::squads_multisig_program::state::ProgramConfig;
use squads_multisig::anchor_lang::AccountDeserialize;

let (program_config_pda, _) = get_program_config_pda(Some(&program::ID));
let account = rpc.get_account(&program_config_pda).await?;
let config = ProgramConfig::try_deserialize(&mut account.data.as_slice())?;
let treasury = config.treasury;
// Now pass `treasury` to MultisigCreateAccountsV2.
```

---

### Bug #2 — Period Type Mismatch (compile error or silent corruption)

**Symptom:** Either a compile-time type mismatch (`expected Period, found u8`)
or silent borsh serialization of a wrong discriminant value.

**Cause:** Passing a raw integer or wrong struct where the `Period` enum is
expected:

```rust
// Wrong — type error or wrong serialization:
period: 1u8

// Wrong — struct not valid here:
period: SpendingPeriod { seconds: 86400 }
```

**Fix:** Use the typed enum variant from `squads_multisig::state`:

```rust
use squads_multisig::state::Period;

period: Period::Day    // Daily reset
period: Period::Week   // Weekly reset
period: Period::Month  // Monthly reset (~30 days)
period: Period::OneTime // Single use, no reset
```

The Cerberus `SpendingPeriod` enum converts to `Period` via `From`:

```rust
use cerberus_skill::spending_account::SpendingPeriod;

let period: Period = SpendingPeriod::Day.into();
```

---

### Bug #3 — configAuthority Doesn't Lock (`multisig still mutable`)

**Symptom:** After TX4, `get_lock_state` reports `config_authority != Pubkey::default()`.
The multisig is still modifiable by the payer.

**Cause:** TX4 was built with the wrong sentinel value:

```rust
// Wrong — leaves payer in control:
config_authority: Some(payer.pubkey())

// Wrong — Option does not exist here:
config_authority: None

// Wrong — wrong address:
config_authority: system_program::id()  // same bytes as default, actually OK
```

**Fix:** Pass `Pubkey::default()` explicitly. The field is `Pubkey`, not
`Option<Pubkey>`:

```rust
use squads_multisig::squads_multisig_program::MultisigSetConfigAuthorityArgs;

MultisigSetConfigAuthorityArgs {
    config_authority: Pubkey::default(), // 32 zero bytes = disabled
    memo: None,
}
```

Verify with `cerberus_skill::verify::assert_fully_locked` after TX4 confirms.

---

## CerberusError Variants

### `CerberusError::InsufficientFunds`

```
insufficient funds: wallet has 5000000 lamports, needs at least 11000000 lamports
```

Fund the payer before setup. Devnet airdrop:

```sh
solana airdrop 0.1 --url devnet
```

### `CerberusError::InvalidSpendingLimitAmount`

```
invalid spending limit: amount must be greater than zero
```

`max_auto_approve_lamports` must be `> 0`. The crate rejects zero before
sending any transactions.

### `CerberusError::LockVerificationFailed`

```
lock verification failed — field `config_authority`: expected `11111111111111111111111111111111`, got `<payer pubkey>`
```

TX4 either did not land or was sent with the wrong sentinel. Call
`recover_partial_setup` to re-send TX4.

### `CerberusError::DeserializationError`

```
account deserialization failed for <addr>: ...
```

Causes:
1. Account is not owned by the Squads v4 program (wrong address passed).
2. `squads-multisig` crate version mismatch with the deployed program version.
3. Account data is corrupted (extremely rare on Solana).

Confirm `account.owner == "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"`.

---

## Squads Program Error Codes

These appear in transaction logs when an instruction is rejected on-chain.

| Error Code | Name | Fix |
|------------|------|-----|
| `0x1770` | `NotEnoughSigners` | Add signatures to meet threshold |
| `0x1771` | `TransactionNotReady` | Time-lock has not expired |
| `0x1772` | `InvalidInstructionArgs` | Check account order and instruction data layout |
| `0x1776` | `InvalidThreshold` | Threshold must be `1..=members.len()` |
| `0x1790` | `SpendingLimitExceeded` | Agent exceeded the per-period cap |
| `0x1791` | `SpendingLimitInvalidMint` | Mint does not match spending limit config |

---

## Build Errors

### Dependency Version Conflicts

`squads-multisig 2.x` requires `solana-sdk ~1.18`. Using `solana-sdk = "2"` in
the same workspace causes `curve25519-dalek` version conflicts:

```
error: failed to select a version for `curve25519-dalek`
```

Fix — pin to 1.18 in `Cargo.toml`:

```toml
[dependencies]
solana-sdk = "1.18"
solana-client = "1.18"
squads-multisig = "2"
```

Run `cargo tree -d` to identify conflicting duplicates.

### `error: the Err-variant returned from this function is very large`

`CerberusError::RpcError` wraps `ClientError`, which is large. The crate boxes
it (`Box<ClientError>`) to satisfy `clippy::result_large_err`. If you add
variants to `CerberusError`, keep large payloads boxed.

# Squads Protocol v4 Bootstrap Pattern (TX1–TX4)

This document describes the four-transaction setup that Cerberus uses to create
a governed, locked spending account. Each transaction has a specific purpose;
skipping or reordering them leaves the account in an insecure state.

## Program ID

```
SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf
```

Same address on mainnet and devnet.

## Why Four Transactions?

The Squads program's design requires sequential, separate transactions for
configuration changes. You cannot atomically create-and-lock in one instruction.

| TX | Instruction | Why it exists |
|----|-------------|---------------|
| TX1 | `multisig_create_v2` | Creates the Multisig PDA with `config_authority = payer`. The payer must have authority to attach a spending limit next. |
| TX2 | `multisig_add_spending_limit` | Attaches the `SpendingLimit` PDA defining the per-period cap. Must happen before locking because only the `config_authority` can call this. |
| TX3 | `multisig_change_threshold` | Explicitly sets threshold to the desired value. Separating this from TX1 makes the intent unambiguous and verifiable. |
| TX4 | `multisig_set_config_authority` | Sets `config_authority = Pubkey::default()` (the all-zeros System Program address). This makes the multisig autonomous — no single key can change configuration anymore. |

After TX4, any configuration change requires a multisig proposal voted on by
`governing_authority`. The `payer` key used during bootstrap has no further power.

## PDA Derivation

```rust
use squads_multisig::pda::{
    get_multisig_pda, get_spending_limit_pda, get_vault_pda, get_program_config_pda,
};
use squads_multisig::squads_multisig_program as program;

// TX1 + TX2 use a fresh ephemeral create_key to derive unique PDAs
let (multisig_pda, _)       = get_multisig_pda(&create_key, Some(&program::ID));
let (vault_pda, _)           = get_vault_pda(&multisig_pda, 0, Some(&program::ID));
let (spending_limit_pda, _) = get_spending_limit_pda(&multisig_pda, &sl_create_key, Some(&program::ID));
let (program_config_pda, _) = get_program_config_pda(Some(&program::ID));
// program_config_pda holds the on-chain treasury address (BUG FIX #1)
```

## Member Permissions

Squads v4 uses a bitmask: `Initiate = 1`, `Vote = 2`, `Execute = 4`.

```rust
use squads_multisig::state::{Member, Permission, Permissions};

// governing_authority: can propose, vote, and execute — full governance
let gov_member = Member {
    key: governing_authority,
    permissions: Permissions::from_vec(&[
        Permission::Initiate,
        Permission::Vote,
        Permission::Execute,
    ]),
};

// agent_wallet: can initiate and execute spending limit use — no Vote
// Agent cannot influence governance even if its key is compromised.
let agent_member = Member {
    key: agent_wallet,
    permissions: Permissions::from_vec(&[
        Permission::Initiate,
        Permission::Execute,
    ]),
};
```

The `Permissions` mask must be `< 8` (only 3 bits are defined).
The multisig invariant requires at least one Voter and `threshold <= num_voters`.

## TX1 — multisig_create_v2

```rust
use squads_multisig::client::{multisig_create_v2, MultisigCreateAccountsV2, MultisigCreateArgsV2};

let ix = multisig_create_v2(
    MultisigCreateAccountsV2 {
        program_config: program_config_pda, // on-chain, not assumed (BUG FIX #1)
        treasury,                           // fetched from ProgramConfig, not payer
        multisig: multisig_pda,
        create_key: create_key.pubkey(),
        creator: payer.pubkey(),
        system_program: system_program::id(),
    },
    MultisigCreateArgsV2 {
        config_authority: Some(payer.pubkey()), // temporary; removed in TX4
        threshold: 1,
        members: vec![gov_member, agent_member],
        time_lock: 0,
        rent_collector: None,
        memo: Some("cerberus-skill bootstrap".to_string()),
    },
    Some(program::ID),
);
```

## TX2 — multisig_add_spending_limit

TX2 is an Anchor instruction not exposed in `squads_multisig::client`.
Use `InstructionData::data()` to build the instruction bytes:

```rust
use squads_multisig::anchor_lang::InstructionData;
use squads_multisig::squads_multisig_program::instruction::MultisigAddSpendingLimit;
use squads_multisig::squads_multisig_program::MultisigAddSpendingLimitArgs;
use squads_multisig::state::Period;

let data = MultisigAddSpendingLimit {
    args: MultisigAddSpendingLimitArgs {
        create_key: sl_create_key,
        vault_index: 0,
        mint: Pubkey::default(),          // Pubkey::default() = native SOL
        amount: max_auto_approve_lamports,
        period: Period::Day,              // typed enum, not raw int (BUG FIX #2)
        members: vec![agent_wallet],
        destinations: vec![],             // empty = any destination
        memo: None,
    },
}
.data();

// Account metas (order matters):
// 0. multisig       — readonly
// 1. config_authority — signer (still payer at this point)
// 2. spending_limit — writable (init)
// 3. rent_payer     — writable signer
// 4. system_program — readonly
```

## TX3 — multisig_change_threshold

```rust
use squads_multisig::squads_multisig_program::instruction::MultisigChangeThreshold;
use squads_multisig::squads_multisig_program::MultisigChangeThresholdArgs;

let data = MultisigChangeThreshold {
    args: MultisigChangeThresholdArgs {
        threshold: 1,
        memo: None,
    },
}
.data();

// Accounts:
// 0. multisig         — writable
// 1. config_authority — signer
// 2. rent_payer       — program::ID placeholder (no rent change needed)
// 3. system_program   — program::ID placeholder
```

## TX4 — multisig_set_config_authority

```rust
use squads_multisig::squads_multisig_program::instruction::MultisigSetConfigAuthority;
use squads_multisig::squads_multisig_program::MultisigSetConfigAuthorityArgs;

let data = MultisigSetConfigAuthority {
    args: MultisigSetConfigAuthorityArgs {
        config_authority: Pubkey::default(), // all-zeros = disabled (BUG FIX #3)
        memo: None,
    },
}
.data();
// NOT None — config_authority is Pubkey, not Option<Pubkey>
```

After TX4, `multisig.config_authority == Pubkey::default()`. This is the locked
state. Verify with `cerberus_skill::verify::assert_fully_locked`.

## The Three Bugs This Pattern Fixes

**BUG FIX #1 — Treasury from ProgramConfig:**
The Squads `multisig_create_v2` instruction requires the protocol treasury address
as an account. This must be fetched from the on-chain `ProgramConfig` PDA, not
assumed to be the payer or a hardcoded address. Using the wrong treasury causes
TX1 to fail with `InvalidTreasury`.

**BUG FIX #2 — Period as typed enum:**
`Period::Day` is a typed Rust enum variant, not an integer. Passing a raw `u8`
where `Period` is expected causes a type mismatch at compile time or silent
serialization corruption at runtime.

**BUG FIX #3 — configAuthority null sentinel:**
To disable `config_authority`, pass `Pubkey::default()` (32 zero bytes = the
System Program address). The field type is `Pubkey`, not `Option<Pubkey>`.
Passing the payer's pubkey instead of `Pubkey::default()` in TX4 leaves the
account modifiable by the payer — the lock is never applied.

## Proof-of-Concept Address (devnet)

Created during Metera development using this exact TX1–TX4 sequence:

```
JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr
```

https://explorer.solana.com/address/JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr?cluster=devnet

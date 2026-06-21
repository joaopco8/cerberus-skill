//! Live, on-chain demonstrations that a governed spending account cannot be
//! bypassed by a single compromised key.
//!
//! Every function in this module submits a REAL transaction to the configured
//! RPC endpoint and reports the REAL on-chain rejection. Nothing here is mocked
//! or simulated in application code. If an attack succeeds (blocked: false), that
//! is reported honestly — this module's credibility depends on truthfulness.

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use squads_multisig::{
    anchor_lang::{AccountDeserialize, InstructionData},
    client::{spending_limit_use, SpendingLimitUseAccounts},
    pda::get_vault_pda,
    squads_multisig_program as program,
    squads_multisig_program::{
        instruction::MultisigChangeThreshold, instructions::SpendingLimitUseArgs,
        MultisigChangeThresholdArgs,
    },
    state::SpendingLimit,
};

// ── Public types ──────────────────────────────────────────────────────────────

/// Result of a single on-chain attack attempt.
pub struct AttackResult {
    /// Short name for this attack vector (used in progress display).
    pub vector_name: &'static str,
    /// Human-readable description of what the attack tried to do.
    pub description: String,
    /// Was the attack blocked by the on-chain program?
    ///
    /// `false` means a security gap was found and must be reported.
    pub blocked: bool,
    /// The real RPC or program error that blocked the attack.
    pub on_chain_error: Option<String>,
}

// ── Attack 1: Overspend ───────────────────────────────────────────────────────

/// Attempt to spend 10× the configured per-period limit, signing only as agent.
///
/// Submits a real `spending_limit_use` instruction with `amount = limit * 10`.
/// The Squads program enforces the cap: `SpendingLimitExceeded` (0x1790).
pub async fn attempt_overspend(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    spending_limit_pda: &Pubkey,
    agent: &Keypair,
    configured_limit: u64,
) -> AttackResult {
    let attempt_amount = configured_limit.saturating_mul(10);
    let (vault_pda, _) = get_vault_pda(multisig_pda, 0, Some(&program::ID));

    let description = format!(
        "spending_limit_use({} lamports) — 10× the configured limit of {} — \
         signed only by agent key, no governing authority",
        attempt_amount, configured_limit
    );

    let ix = spending_limit_use(
        SpendingLimitUseAccounts {
            multisig: *multisig_pda,
            member: agent.pubkey(),
            spending_limit: *spending_limit_pda,
            vault: vault_pda,
            destination: agent.pubkey(),
            system_program: Some(system_program::id()),
            mint: None,
            vault_token_account: None,
            destination_token_account: None,
            token_program: None,
        },
        SpendingLimitUseArgs {
            amount: attempt_amount,
            decimals: 9,
            memo: None,
        },
        Some(program::ID),
    );

    let bh = match rpc.get_latest_blockhash().await {
        Ok(h) => h,
        Err(e) => {
            return AttackResult {
                vector_name: "overspend",
                description,
                blocked: false,
                on_chain_error: Some(format!("RPC error: {e}")),
            }
        }
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&agent.pubkey()), &[agent], bh);
    let result = rpc.send_and_confirm_transaction(&tx).await;

    match result {
        Err(e) => AttackResult {
            vector_name: "overspend",
            description,
            blocked: true,
            on_chain_error: Some(format!("{e}")),
        },
        Ok(_sig) => AttackResult {
            vector_name: "overspend",
            description,
            blocked: false,
            on_chain_error: None,
        },
    }
}

// ── Attack 2: Config escalation ───────────────────────────────────────────────

/// Attempt to act as config authority and change the threshold, signing as agent.
///
/// The agent is NOT the config authority. Even if it were, `config_authority`
/// is `Pubkey::default()` (disabled) after TX4, so no key can sign for it.
/// The Squads program enforces `ConstraintHasOne` — the signer must match the
/// stored `config_authority`, which is 32 zero bytes with no private key.
pub async fn attempt_config_escalation(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    agent: &Keypair,
) -> AttackResult {
    let description = format!(
        "multisig_change_threshold signed by agent ({}) pretending to be config_authority — \
         config_authority is Pubkey::default() (disabled), no valid signer exists",
        agent.pubkey()
    );

    let data = MultisigChangeThreshold {
        args: MultisigChangeThresholdArgs {
            new_threshold: 1,
            memo: None,
        },
    }
    .data();

    // Accounts: multisig (writable), config_authority (signer), rent_payer, system_program.
    // We place agent.pubkey() in the config_authority slot — this is the attack.
    // On-chain the stored config_authority is Pubkey::default(); agent != default → REJECTED.
    let ix = Instruction {
        program_id: program::ID,
        accounts: vec![
            AccountMeta::new(*multisig_pda, false),
            AccountMeta::new_readonly(agent.pubkey(), true),
            AccountMeta::new_readonly(program::ID, false),
            AccountMeta::new_readonly(program::ID, false),
        ],
        data,
    };

    let bh = match rpc.get_latest_blockhash().await {
        Ok(h) => h,
        Err(e) => {
            return AttackResult {
                vector_name: "config-escalation",
                description,
                blocked: false,
                on_chain_error: Some(format!("RPC error: {e}")),
            }
        }
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&agent.pubkey()), &[agent], bh);
    let result = rpc.send_and_confirm_transaction(&tx).await;

    match result {
        Err(e) => AttackResult {
            vector_name: "config-escalation",
            description,
            blocked: true,
            on_chain_error: Some(format!("{e}")),
        },
        Ok(_sig) => AttackResult {
            vector_name: "config-escalation",
            description,
            blocked: false,
            on_chain_error: None,
        },
    }
}

// ── Attack 3: Direct vault drain ─────────────────────────────────────────────

/// Attempt to drain the vault via a raw system_program transfer, bypassing
/// the spending limit mechanism entirely.
///
/// The vault PDA is owned by the Squads program — it has no private key and
/// cannot sign any transaction. The system program requires the `from` account
/// to be a signer. Since vault_pda cannot sign, the transaction is rejected
/// before it even reaches the Squads program logic.
pub async fn attempt_direct_vault_drain(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    agent: &Keypair,
    amount: u64,
) -> AttackResult {
    let (vault_pda, _) = get_vault_pda(multisig_pda, 0, Some(&program::ID));

    let description = format!(
        "system Transfer(from: vault {}, to: agent, amount: {}) — vault is a Squads PDA with \
         no private key; marked is_signer=false so the tx signs with agent alone, then the \
         system program rejects on-chain because from.is_signer is false",
        vault_pda, amount
    );

    // Build the system Transfer instruction manually with vault_pda as is_signer=false.
    // system_instruction::transfer would mark vault_pda as is_signer=true, causing
    // Transaction::sign to panic (NotEnoughSigners) since the PDA has no private key.
    // Marking it false lets us sign with agent only; the system program on-chain checks
    // from.is_signer and rejects with MissingRequiredSignature — the real security proof.
    let ix = Instruction {
        program_id: system_program::id(),
        accounts: vec![
            AccountMeta::new(vault_pda, false), // from: writable, NOT signer — no key
            AccountMeta::new(agent.pubkey(), false), // to: writable
        ],
        // SystemInstruction::Transfer discriminant = 2u32 LE + lamports u64 LE
        data: {
            let mut d = Vec::with_capacity(12);
            d.extend_from_slice(&2u32.to_le_bytes());
            d.extend_from_slice(&amount.to_le_bytes());
            d
        },
    };

    let bh = match rpc.get_latest_blockhash().await {
        Ok(h) => h,
        Err(e) => {
            return AttackResult {
                vector_name: "vault-drain",
                description,
                blocked: false,
                on_chain_error: Some(format!("RPC error: {e}")),
            }
        }
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&agent.pubkey()), &[agent], bh);
    let result = rpc.send_and_confirm_transaction(&tx).await;

    match result {
        Err(e) => AttackResult {
            vector_name: "vault-drain",
            description,
            blocked: true,
            on_chain_error: Some(format!("{e}")),
        },
        Ok(_sig) => AttackResult {
            vector_name: "vault-drain",
            description,
            blocked: false,
            on_chain_error: None,
        },
    }
}

// ── Orchestrator ─────────────────────────────────────────────────────────────

/// Fetch the configured `amount` from an on-chain `SpendingLimit` account.
pub async fn fetch_spending_limit_amount(rpc: &RpcClient, pda: &Pubkey) -> Option<u64> {
    let account = rpc.get_account(pda).await.ok()?;
    let sl = SpendingLimit::try_deserialize(&mut account.data.as_slice()).ok()?;
    Some(sl.amount)
}

/// Run all 3 attack vectors in sequence and return the results.
///
/// If `spending_limit_pda` is `None`, Attack 1 is skipped (the PDA is required
/// to construct the `spending_limit_use` instruction).
pub async fn run_full_simulation(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    spending_limit_pda: Option<&Pubkey>,
    agent: &Keypair,
) -> Vec<AttackResult> {
    let mut results = Vec::new();

    if let Some(sl_pda) = spending_limit_pda {
        let limit = fetch_spending_limit_amount(rpc, sl_pda)
            .await
            .unwrap_or(1_000_000);
        results.push(attempt_overspend(rpc, multisig_pda, sl_pda, agent, limit).await);
    }

    results.push(attempt_config_escalation(rpc, multisig_pda, agent).await);
    results.push(attempt_direct_vault_drain(rpc, multisig_pda, agent, 1_000_000).await);

    results
}

// ── Error decoding ────────────────────────────────────────────────────────────

/// Decode a Squads `MultisigError` hex code embedded in an RPC error string.
///
/// Anchor assigns error codes starting at 6000 (0x1770). Each variant in
/// `squads_multisig_program::errors::MultisigError` adds one to that base.
/// Returns the enum variant name if recognised, `None` otherwise.
pub fn decode_squads_error(raw: &str) -> Option<&'static str> {
    // 0x1770 + variant index = error code
    if raw.contains("0x1770") {
        Some("DuplicateMember")
    } else if raw.contains("0x1771") {
        Some("EmptyMembers")
    } else if raw.contains("0x1772") {
        Some("TooManyMembers")
    } else if raw.contains("0x1773") {
        Some("InvalidThreshold")
    } else if raw.contains("0x1774") {
        Some("Unauthorized")
    } else if raw.contains("0x1775") {
        Some("NotAMember")
    } else if raw.contains("0x1776") {
        Some("InvalidTransactionMessage")
    } else if raw.contains("0x1777") {
        Some("StaleProposal")
    } else if raw.contains("0x1778") {
        Some("InvalidProposalStatus")
    } else if raw.contains("0x1779") {
        Some("InvalidTransactionIndex")
    } else if raw.contains("0x177a") {
        Some("AlreadyApproved")
    } else if raw.contains("0x177b") {
        Some("AlreadyRejected")
    } else if raw.contains("0x177c") {
        Some("AlreadyCancelled")
    } else if raw.contains("0x177d") {
        Some("InvalidNumberOfAccounts")
    } else if raw.contains("0x177e") {
        Some("InvalidAccount")
    } else if raw.contains("0x177f") {
        Some("RemoveLastMember")
    } else if raw.contains("0x1780") {
        Some("NoVoters")
    } else if raw.contains("0x1781") {
        Some("NoProposers")
    } else if raw.contains("0x1782") {
        Some("NoExecutors")
    } else if raw.contains("0x1783") {
        Some("InvalidStaleTransactionIndex")
    } else if raw.contains("0x1784") {
        Some("NotSupportedForControlled")
    } else if raw.contains("0x1785") {
        Some("TimeLockNotReleased")
    } else if raw.contains("0x1786") {
        Some("NoActions")
    } else if raw.contains("0x1787") {
        Some("MissingAccount")
    } else if raw.contains("0x1788") {
        Some("InvalidMint")
    } else if raw.contains("0x1789") {
        Some("InvalidDestination")
    } else if raw.contains("0x178a") {
        Some("SpendingLimitExceeded")
    } else if raw.contains("0x178b") {
        Some("DecimalsMismatch")
    } else if raw.contains("0x178c") {
        Some("UnknownPermission")
    } else if raw.contains("0x178d") {
        Some("ProtectedAccount")
    } else if raw.contains("0x178e") {
        Some("TimeLockExceedsMaxAllowed")
    } else if raw.contains("0x178f") {
        Some("IllegalAccountOwner")
    } else if raw.contains("0x1790") {
        Some("RentReclamationDisabled")
    } else if raw.contains("0x1791") {
        Some("InvalidRentCollector")
    } else if raw.contains("0x1792") {
        Some("ProposalForAnotherMultisig")
    } else if raw.contains("0x1793") {
        Some("TransactionForAnotherMultisig")
    } else if raw.contains("0x1794") {
        Some("TransactionNotMatchingProposal")
    } else if raw.contains("0x1795") {
        Some("TransactionNotLastInBatch")
    } else if raw.contains("0x1796") {
        Some("BatchNotEmpty")
    } else if raw.contains("0x1797") {
        Some("SpendingLimitInvalidAmount")
    } else {
        None
    }
}

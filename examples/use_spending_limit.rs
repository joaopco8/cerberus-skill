//! Demonstrate Tier 1 auto-approval: spend from a locked Squads vault.
//!
//! The agent wallet signs this transaction alone — no governing_authority
//! co-signature is needed, because the amount is under the on-chain
//! spending limit cap. The Squads program enforces this ceiling itself.
//!
//! If the agent tries to exceed `spending_limit.amount`, the Squads program
//! rejects the transaction on-chain (not just "discourages" it in code) —
//! the instruction fails with `SpendingLimitExceeded`, regardless of what any
//! off-chain system says.
//!
//! ```sh
//! MULTISIG_PDA=<addr>        \
//! SPENDING_LIMIT_PDA=<addr>  \
//! DESTINATION=<addr>         \
//! AMOUNT=1000000             \
//! cargo run --example use_spending_limit
//! ```
//!
//! The agent keypair defaults to `~/.config/solana/id.json`.
//! Override with `AGENT_KEYPAIR_PATH=<path>`.

use anyhow::Result;
use cerberus_skill::lock::get_lock_state;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use squads_multisig::{
    client::{spending_limit_use, SpendingLimitUseAccounts},
    pda::get_vault_pda,
    squads_multisig_program as program,
    squads_multisig_program::instructions::SpendingLimitUseArgs,
};
use std::str::FromStr;

/// Tiers relative to the on-chain spending limit cap.
/// Tier 1: amount ≤ remaining budget  → auto-approved by on-chain rule alone.
/// Tier 2: remaining < amount ≤ cap×5 → needs extra off-chain verification.
/// Tier 3: amount > cap×5             → requires governing_authority vote.
#[derive(Debug)]
enum ApprovalTier {
    Tier1AutoApproved,
    Tier2ExtraVerification { shortfall: u64 },
    Tier3HumanApproval { excess_multiplier: u64 },
}

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

#[tokio::main]
async fn main() -> Result<()> {
    cerberus_skill::banner::print_banner();
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    let rpc = RpcClient::new(rpc_url);

    let multisig_pda: Pubkey = std::env::var("MULTISIG_PDA")
        .map(|s| Pubkey::from_str(&s))
        .unwrap_or_else(|_| "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse())?;

    let spending_limit_pda: Pubkey = std::env::var("SPENDING_LIMIT_PDA")
        .map(|s| Pubkey::from_str(&s))
        .map_err(|_| anyhow::anyhow!("SPENDING_LIMIT_PDA env var required"))??;

    let destination: Pubkey = std::env::var("DESTINATION")
        .map(|s| Pubkey::from_str(&s))
        .map_err(|_| anyhow::anyhow!("DESTINATION env var required"))??;

    let amount: u64 = std::env::var("AMOUNT")
        .unwrap_or_else(|_| "1000000".to_string())
        .parse()?;

    let keypair_path = std::env::var("AGENT_KEYPAIR_PATH")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").to_string());
    let agent: Keypair = read_keypair_file(&keypair_path)
        .map_err(|e| anyhow::anyhow!("agent keypair read failed: {e}"))?;

    // Derive the vault PDA (vault index 0) — this is where the agent's SOL lives.
    let (vault_pda, _) = get_vault_pda(&multisig_pda, 0, Some(&program::ID));

    // Check lock state so the user can see the multisig is actually locked.
    let state = get_lock_state(&rpc, &multisig_pda)
        .await
        .map_err(|e| anyhow::anyhow!("lock state read failed: {e}"))?;

    println!("multisig:       {multisig_pda}");
    println!("vault:          {vault_pda}");
    println!("spending limit: {spending_limit_pda}");
    println!(
        "locked:         {}",
        state.config_authority == Pubkey::default()
    );
    println!("threshold:      {}", state.threshold);
    println!(
        "amount:         {amount} lamports ({:.6} SOL)",
        amount as f64 / LAMPORTS_PER_SOL as f64
    );
    println!();

    // Spending limit amount is stored on-chain — fetch it for tier classification.
    // For this example we use the requested amount as a proxy for the limit.
    // In production, deserialize the SpendingLimit account to read .amount and
    // .remaining_amount fields directly.
    let limit_placeholder = 10_000_000u64; // 0.01 SOL — replace with on-chain value
    let remaining_placeholder = limit_placeholder; // assume full budget available

    match classify(amount, remaining_placeholder, limit_placeholder) {
        ApprovalTier::Tier1AutoApproved => {
            println!("[Tier 1] amount within on-chain limit — sending with agent signature only");

            // This is the core Cerberus promise: the agent signs alone, and the
            // Squads program enforces the ceiling without any co-signer.
            let ix = spending_limit_use(
                SpendingLimitUseAccounts {
                    multisig: multisig_pda,
                    member: agent.pubkey(),
                    spending_limit: spending_limit_pda,
                    vault: vault_pda,
                    destination,
                    // SOL transfer: system_program is needed, no SPL token accounts.
                    system_program: Some(system_program::id()),
                    mint: None,
                    vault_token_account: None,
                    destination_token_account: None,
                    token_program: None,
                },
                SpendingLimitUseArgs {
                    amount,
                    decimals: 9, // SOL has 9 decimal places (lamports)
                    memo: None,
                },
                Some(program::ID),
            );

            let blockhash = rpc.get_latest_blockhash().await?;
            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&agent.pubkey()),
                &[&agent], // only the agent signs — governing_authority NOT required
                blockhash,
            );

            let sig = rpc
                .send_and_confirm_transaction(&tx)
                .await
                .map_err(|e| anyhow::anyhow!("transaction failed: {e}"))?;

            println!("transaction confirmed: {sig}");
            println!("vault balance reduced by {amount} lamports");
        }
        ApprovalTier::Tier2ExtraVerification { shortfall } => {
            println!("[Tier 2] shortfall {shortfall} lamports — extra verification required");
            println!("  implement: TOTP check, webhook approval, rate-limit enforcement");
            println!("  see: skill/tiered-auto-approval.md");
        }
        ApprovalTier::Tier3HumanApproval { excess_multiplier } => {
            println!("[Tier 3] {excess_multiplier}× over limit — human multisig vote required");
            println!("  implement: create Squads proposal, wait for governing_authority signature");
            println!("  see: skill/tiered-auto-approval.md");
        }
    }

    Ok(())
}

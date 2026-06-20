//! Recovery tool: resume an interrupted TX1–TX4 setup and confirm the lock.
//!
//! If setup was interrupted after TX1 (multisig exists) but before TX3/TX4
//! (configAuthority not yet disabled), this example detects the gap and
//! completes the missing transactions automatically.
//!
//! ```sh
//! MULTISIG_PDA=<address> cargo run --example finalize_lock
//! ```
//!
//! If the multisig is already fully locked, this is a no-op — safe to run
//! repeatedly as a health check.

use anyhow::Result;
use cerberus_skill::{lock::get_lock_state, recover::recover_partial_setup};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::read_keypair_file};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    let rpc = RpcClient::new(rpc_url);

    let multisig_pda = std::env::var("MULTISIG_PDA")
        .map(|s| Pubkey::from_str(&s))
        .unwrap_or_else(|_| "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse())?;

    // Payer must be the original bootstrap configAuthority (the same keypair
    // used as `payer` when create_governed_spending_account was first called).
    let keypair_path = std::env::var("KEYPAIR_PATH")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").to_string());
    let payer = read_keypair_file(&keypair_path)
        .map_err(|e| anyhow::anyhow!("keypair read failed: {e}"))?;

    // Check current state before attempting recovery.
    let before = get_lock_state(&rpc, &multisig_pda)
        .await
        .map_err(|e| anyhow::anyhow!("cannot read lock state: {e}"))?;

    if before.is_fully_locked(1) {
        println!("multisig {multisig_pda} is already fully locked");
        println!("  config_authority: {} (disabled)", before.config_authority);
        println!("  threshold:        {}", before.threshold);
        return Ok(());
    }

    println!("partial setup detected for {multisig_pda}");
    println!("  config_authority: {}", before.config_authority);
    println!("  threshold:        {}", before.threshold);
    println!("  recovering...");

    // recover_partial_setup sends only the missing transactions.
    // Idempotent: safe to call even if some steps already applied.
    recover_partial_setup(&rpc, &payer, &multisig_pda, 1)
        .await
        .map_err(|e| anyhow::anyhow!("recovery failed: {e}"))?;

    let after = get_lock_state(&rpc, &multisig_pda)
        .await
        .map_err(|e| anyhow::anyhow!("post-recovery state read failed: {e}"))?;

    println!("recovery complete");
    println!("  config_authority: {} (disabled)", after.config_authority);
    println!("  threshold:        {}", after.threshold);

    Ok(())
}

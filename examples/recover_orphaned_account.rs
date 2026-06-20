//! Batch recovery: scan a list of multisig PDAs and finalize any incomplete setups.
//!
//! Reads multisig PDA addresses from a file (one per line), runs
//! `recover_partial_setup` on each, and prints a summary table.
//!
//! This mirrors the real production recovery script pattern — useful when
//! deploying Cerberus at scale and needing to audit/recover a batch of accounts
//! after a network outage or service interruption.
//!
//! ```sh
//! PDAS_FILE=multisigs.txt cargo run --example recover_orphaned_account
//! ```
//!
//! `multisigs.txt` format (one base58 PDA per line, blank lines and `#`
//! comments are ignored):
//! ```text
//! # My agent fleet
//! JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr
//! AnotherPDAHere...
//! ```
//!
//! The payer keypair must be the original bootstrap configAuthority for every
//! address in the file. Set `KEYPAIR_PATH` to override `~/.config/solana/id.json`.

use anyhow::Result;
use cerberus_skill::{lock::get_lock_state, recover::recover_partial_setup};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::read_keypair_file};
use std::str::FromStr;

#[derive(Debug)]
enum RecoveryStatus {
    AlreadyLocked,
    Recovered,
    Failed(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    cerberus_skill::banner::print_banner();
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    let rpc = RpcClient::new(rpc_url);

    let pdas_file = std::env::var("PDAS_FILE").unwrap_or_else(|_| "multisigs.txt".to_string());

    let keypair_path = std::env::var("KEYPAIR_PATH")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").to_string());
    let payer = read_keypair_file(&keypair_path)
        .map_err(|e| anyhow::anyhow!("keypair read failed ({keypair_path}): {e}"))?;

    // Read PDA list. Skip blank lines and comments.
    let contents = std::fs::read_to_string(&pdas_file)
        .map_err(|e| anyhow::anyhow!("cannot read {pdas_file}: {e}"))?;

    let pdas: Vec<Pubkey> = contents
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| Pubkey::from_str(l).map_err(|e| anyhow::anyhow!("invalid PDA '{l}': {e}")))
        .collect::<Result<Vec<_>>>()?;

    if pdas.is_empty() {
        println!("no PDAs found in {pdas_file}");
        return Ok(());
    }

    println!("scanning {} multisig(s) from {pdas_file}", pdas.len());
    println!();

    let mut results: Vec<(Pubkey, RecoveryStatus)> = Vec::with_capacity(pdas.len());

    for pda in &pdas {
        let status = match get_lock_state(&rpc, pda).await {
            Err(e) => RecoveryStatus::Failed(format!("cannot fetch state: {e}")),
            Ok(state) if state.is_fully_locked(1) => RecoveryStatus::AlreadyLocked,
            Ok(_) => match recover_partial_setup(&rpc, &payer, pda, 1).await {
                Ok(()) => RecoveryStatus::Recovered,
                Err(e) => RecoveryStatus::Failed(format!("recovery failed: {e}")),
            },
        };

        // Print result as it's computed, so the user sees progress.
        let label = match &status {
            RecoveryStatus::AlreadyLocked => "OK     ",
            RecoveryStatus::Recovered => "FIXED  ",
            RecoveryStatus::Failed(_) => "FAILED ",
        };
        println!("{label} {pda}");
        if let RecoveryStatus::Failed(msg) = &status {
            println!("       {msg}");
        }

        results.push((*pda, status));
    }

    // Summary
    let locked = results
        .iter()
        .filter(|(_, s)| matches!(s, RecoveryStatus::AlreadyLocked))
        .count();
    let recovered = results
        .iter()
        .filter(|(_, s)| matches!(s, RecoveryStatus::Recovered))
        .count();
    let failed = results
        .iter()
        .filter(|(_, s)| matches!(s, RecoveryStatus::Failed(_)))
        .count();

    println!();
    println!(
        "summary: {} ok / {} recovered / {} failed",
        locked, recovered, failed
    );

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

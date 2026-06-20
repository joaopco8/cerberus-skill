//! Create a Squads v4 governed spending account for an AI agent wallet.
//!
//! Runs TX1–TX4 (bootstrap multisig → add spending limit → set threshold →
//! lock configAuthority) and prints the resulting PDA addresses.
//!
//! ```sh
//! cargo run --example create_agent_spending_account -- \
//!   --agent-wallet <PUBKEY> \
//!   --keypair ~/.config/solana/id.json \
//!   --max-auto-approve 0.01 \
//!   --rpc-url https://api.devnet.solana.com
//! ```
//!
//! Fund the payer first:
//! ```sh
//! solana airdrop 0.1 --url devnet
//! ```

use anyhow::{anyhow, Result};
use cerberus_skill::spending_account::{
    create_governed_spending_account, SpendingPeriod, SpendingTierConfig,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};
use std::str::FromStr;

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

struct Args {
    agent_wallet: Pubkey,
    keypair_path: String,
    max_auto_approve_lamports: u64,
    rpc_url: String,
}

fn parse_args() -> Result<Args> {
    let raw: Vec<String> = std::env::args().collect();
    let mut agent_wallet = None::<Pubkey>;
    let mut keypair_path = None::<String>;
    let mut max_auto_approve_sol = None::<f64>;
    let mut rpc_url = "https://api.devnet.solana.com".to_string();

    let mut i = 1usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "--agent-wallet" => {
                i += 1;
                agent_wallet = Some(
                    Pubkey::from_str(
                        raw.get(i)
                            .ok_or_else(|| anyhow!("--agent-wallet needs a value"))?,
                    )
                    .map_err(|e| anyhow!("invalid pubkey: {e}"))?,
                );
            }
            "--keypair" => {
                i += 1;
                keypair_path = Some(
                    shellexpand::tilde(
                        raw.get(i)
                            .ok_or_else(|| anyhow!("--keypair needs a value"))?,
                    )
                    .to_string(),
                );
            }
            "--max-auto-approve" => {
                i += 1;
                max_auto_approve_sol = Some(
                    raw.get(i)
                        .ok_or_else(|| anyhow!("--max-auto-approve needs a value"))?
                        .parse::<f64>()
                        .map_err(|e| anyhow!("invalid SOL amount: {e}"))?,
                );
            }
            "--rpc-url" => {
                i += 1;
                rpc_url = raw
                    .get(i)
                    .ok_or_else(|| anyhow!("--rpc-url needs a value"))?
                    .clone();
            }
            flag => return Err(anyhow!("unknown flag: {flag}")),
        }
        i += 1;
    }

    Ok(Args {
        agent_wallet: agent_wallet.ok_or_else(|| anyhow!("--agent-wallet is required"))?,
        keypair_path: keypair_path
            .unwrap_or_else(|| shellexpand::tilde("~/.config/solana/id.json").to_string()),
        max_auto_approve_lamports: ((max_auto_approve_sol
            .ok_or_else(|| anyhow!("--max-auto-approve is required"))?)
            * LAMPORTS_PER_SOL) as u64,
        rpc_url,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    let payer: Keypair = read_keypair_file(&args.keypair_path)
        .map_err(|e| anyhow!("keypair read failed ({p}): {e}", p = args.keypair_path))?;

    println!("payer:        {}", payer.pubkey());
    println!("agent wallet: {}", args.agent_wallet);
    println!(
        "limit:        {} lamports ({:.4} SOL) / day",
        args.max_auto_approve_lamports,
        args.max_auto_approve_lamports as f64 / LAMPORTS_PER_SOL
    );
    println!("rpc:          {}", args.rpc_url);
    println!();

    let rpc = RpcClient::new(args.rpc_url.clone());

    // The governing_authority here is the payer themselves.
    // In production you'd pass a separate hardware-wallet pubkey.
    let governing_authority = payer.pubkey();

    let account = create_governed_spending_account(
        &rpc,
        &payer,
        args.agent_wallet,
        governing_authority,
        SpendingTierConfig {
            max_auto_approve_lamports: args.max_auto_approve_lamports,
            period: SpendingPeriod::Day,
            mint: solana_sdk::pubkey::Pubkey::default(), // native SOL
        },
    )
    .await
    .map_err(|e| anyhow!("setup failed: {e}"))?;

    println!("setup complete (TX1-TX4 confirmed)");
    println!();
    println!("  multisig PDA:       {}", account.multisig_pda);
    println!("  vault PDA:          {}", account.vault_pda);
    println!("  spending limit PDA: {}", account.spending_limit_pda);

    let cluster = if args.rpc_url.contains("devnet") {
        "?cluster=devnet"
    } else if args.rpc_url.contains("mainnet") {
        ""
    } else {
        "?cluster=custom"
    };

    println!();
    println!("Solana Explorer:");
    println!(
        "  https://explorer.solana.com/address/{}{cluster}",
        account.multisig_pda
    );

    Ok(())
}

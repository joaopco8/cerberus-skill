//! `cerberus` CLI binary.
//!
//! ```sh
//! cerberus init   --agent-wallet <PUBKEY> --max-auto-approve <LAMPORTS>
//! cerberus status <MULTISIG_PDA>
//! cerberus recover <MULTISIG_PDA>
//! ```

use cerberus_skill::{
    banner,
    cli::{Cli, Commands},
    error::CerberusError,
    lock::get_lock_state,
    recover::recover_partial_setup,
    spending_account::{create_governed_spending_account, SpendingPeriod, SpendingTierConfig},
};
use clap::Parser;
use owo_colors::OwoColorize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};
use squads_multisig::{pda::get_vault_pda, squads_multisig_program as program};

#[tokio::main]
async fn main() {
    banner::print_banner();
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init {
            agent_wallet,
            max_auto_approve,
            mint,
        } => {
            run_init(
                &cli.rpc_url,
                &cli.keypair,
                &agent_wallet,
                max_auto_approve,
                mint,
            )
            .await
        }
        Commands::Status { multisig_pda } => run_status(&cli.rpc_url, &multisig_pda).await,
        Commands::Recover {
            multisig_pda,
            expected_threshold,
        } => {
            run_recover(
                &cli.rpc_url,
                &cli.keypair,
                &multisig_pda,
                expected_threshold,
            )
            .await
        }
    };

    if let Err(e) = result {
        eprintln!("\n{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_keypair(path: &str) -> anyhow::Result<Keypair> {
    let expanded = shellexpand::tilde(path).to_string();
    read_keypair_file(&expanded)
        .map_err(|e| anyhow::anyhow!("keypair read failed ({expanded}): {e}"))
}

fn parse_pubkey(s: &str, label: &str) -> anyhow::Result<Pubkey> {
    s.parse::<Pubkey>()
        .map_err(|e| anyhow::anyhow!("invalid {label} pubkey '{s}': {e}"))
}

// ── run_init ──────────────────────────────────────────────────────────────────

async fn run_init(
    rpc_url: &str,
    keypair_path: &str,
    agent_wallet_str: &str,
    max_auto_approve: u64,
    mint_str: Option<String>,
) -> anyhow::Result<()> {
    let rpc = RpcClient::new(rpc_url.to_string());
    let payer = load_keypair(keypair_path)?;
    let agent_wallet = parse_pubkey(agent_wallet_str, "agent-wallet")?;
    let mint = mint_str
        .as_deref()
        .map(|s| parse_pubkey(s, "mint"))
        .transpose()?
        .unwrap_or_default();

    let governing_authority = payer.pubkey();

    println!(
        "  payer (governing authority): {}",
        governing_authority.to_string().dimmed()
    );
    println!(
        "  agent wallet:                {}",
        agent_wallet.to_string().dimmed()
    );
    println!(
        "  limit:                       {} lamports / day",
        max_auto_approve
    );
    println!(
        "  mint:                        {}",
        if mint == Pubkey::default() {
            "native SOL".to_string()
        } else {
            mint.to_string()
        }
    );
    println!();
    println!("  Sending TX1–TX4 …");
    println!();

    let account = create_governed_spending_account(
        &rpc,
        &payer,
        agent_wallet,
        governing_authority,
        SpendingTierConfig {
            max_auto_approve_lamports: max_auto_approve,
            period: SpendingPeriod::Day,
            mint,
        },
    )
    .await
    .map_err(|e: CerberusError| anyhow::anyhow!("{e}"))?;

    println!(
        "  {} setup complete (TX1–TX4 confirmed)",
        "✓".green().bold()
    );
    println!();
    println!(
        "  Multisig PDA:       {}",
        account.multisig_pda.to_string().bold()
    );
    println!(
        "  Vault PDA:          {}",
        account.vault_pda.to_string().bold()
    );
    println!(
        "  Spending Limit PDA: {}",
        account.spending_limit_pda.to_string().bold()
    );
    println!();
    println!(
        "  {} Fund the vault before the agent can spend:",
        "Next:".bold()
    );
    println!(
        "       solana transfer {} {} --url {}",
        account.vault_pda, max_auto_approve, rpc_url
    );

    Ok(())
}

// ── run_status ────────────────────────────────────────────────────────────────

async fn run_status(rpc_url: &str, multisig_pda_str: &str) -> anyhow::Result<()> {
    let rpc = RpcClient::new(rpc_url.to_string());
    let multisig_pda = parse_pubkey(multisig_pda_str, "multisig-pda")?;
    let (vault_pda, _) = get_vault_pda(&multisig_pda, 0, Some(&program::ID));

    let state = get_lock_state(&rpc, &multisig_pda)
        .await
        .map_err(|e: CerberusError| anyhow::anyhow!("{e}"))?;

    let is_locked = state.config_authority == Pubkey::default();

    println!("  Multisig:          {}", multisig_pda.to_string().bold());
    println!("  Vault (index 0):   {}", vault_pda.to_string().bold());
    println!();

    if is_locked {
        println!(
            "  Config Authority:  {} Disabled (locked)",
            "✓".green().bold()
        );
        println!(
            "  Threshold:         {}/{} (fully governed)",
            state.threshold, state.threshold
        );
        println!();
        println!(
            "  Status: {} — no single key can alter spending limits",
            "SECURE".green().bold()
        );
    } else {
        println!(
            "  Config Authority:  {} ACTIVE ({})",
            "⚠".yellow().bold(),
            state.config_authority
        );
        println!(
            "  Threshold:         {}/{}",
            state.threshold, state.threshold
        );
        println!();
        println!(
            "  Status: {} — bootstrap authority still has unilateral control",
            "INSECURE".red().bold()
        );
        println!(
            "          Run {} to fix.",
            format!("`cerberus recover {multisig_pda_str}`").yellow()
        );
    }

    Ok(())
}

// ── run_recover ───────────────────────────────────────────────────────────────

async fn run_recover(
    rpc_url: &str,
    keypair_path: &str,
    multisig_pda_str: &str,
    expected_threshold: u16,
) -> anyhow::Result<()> {
    let rpc = RpcClient::new(rpc_url.to_string());
    let payer = load_keypair(keypair_path)?;
    let multisig_pda = parse_pubkey(multisig_pda_str, "multisig-pda")?;

    let before = get_lock_state(&rpc, &multisig_pda)
        .await
        .map_err(|e: CerberusError| anyhow::anyhow!("{e}"))?;

    if before.is_fully_locked(expected_threshold) {
        println!(
            "  {} {} is already fully locked — nothing to do.",
            "✓".green().bold(),
            multisig_pda_str.dimmed()
        );
        return Ok(());
    }

    println!("  Partial setup detected:");
    println!("    config_authority: {}", before.config_authority);
    println!("    threshold:        {}", before.threshold);
    println!("  Recovering …");

    recover_partial_setup(&rpc, &payer, &multisig_pda, expected_threshold)
        .await
        .map_err(|e: CerberusError| anyhow::anyhow!("{e}"))?;

    println!(
        "  {} recovery complete — multisig is now fully locked.",
        "✓".green().bold()
    );
    Ok(())
}

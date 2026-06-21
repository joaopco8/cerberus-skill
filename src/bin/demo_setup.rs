//! Bootstrap a governed spending account for the attack simulation demo.
//!
//! Persists the payer keypair to ./demo-payer.json so SOL sent to it is not
//! wasted on subsequent runs. Supports funding via transfer from a local
//! keypair file (--fund-from) to avoid the rate-limited public airdrop faucet.
//!
//! All output is also written to ./demo-setup.log for environments where
//! stdout capture is unavailable.
//!
//! ```sh
//! # First run — generates and saves payer, then airdrops
//! cargo run --bin demo_setup
//!
//! # Fund from Metera devnet keypair instead of airdrop
//! cargo run --bin demo_setup -- --fund-from /path/to/metera-devnet.json
//! ```

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;

use cerberus_skill::spending_account::{
    create_governed_spending_account, SpendingPeriod, SpendingTierConfig,
};
use clap::Parser;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{read_keypair_file, write_keypair_file, Keypair},
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};

const LIMIT_LAMPORTS: u64 = 1_000_000; // 0.001 SOL per day
const VAULT_FUND: u64 = 2_000_000; // 0.002 SOL — enough for legit Tier 1 test
const AGENT_FUND: u64 = 10_000_000; // 0.01 SOL — tx fees for simulate-attack
const FUND_LAMPORTS: u64 = LAMPORTS_PER_SOL / 2; // 0.5 SOL
const PAYER_PATH: &str = "./demo-payer.json";
const LOG_PATH: &str = "./demo-setup.log";

macro_rules! tee {
    ($log:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        let _ = writeln!($log, "{}", msg);
        let _ = $log.flush();
    }};
}

#[derive(Parser)]
#[command(about = "Bootstrap a cerberus demo governed spending account")]
struct Cli {
    /// Path to a funded keypair JSON file to transfer SOL from instead of using airdrop
    #[arg(long, value_name = "KEYPAIR_FILE")]
    fund_from: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
        .expect("failed to open log file");

    writeln!(log, "\n=== demo_setup run ===").unwrap();
    log.flush().unwrap();

    if let Err(e) = run(cli, &mut log).await {
        let msg = format!("  ERROR: {e:#}");
        eprintln!("{msg}");
        writeln!(log, "{msg}").unwrap();
        log.flush().unwrap();
        std::process::exit(1);
    }
}

async fn run(cli: Cli, log: &mut std::fs::File) -> anyhow::Result<()> {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );

    // Load or generate persisted payer keypair
    let payer_path = PathBuf::from(PAYER_PATH);
    let payer = if payer_path.exists() {
        let kp = read_keypair_file(&payer_path)
            .map_err(|e| anyhow::anyhow!("failed to read payer keypair: {e}"))?;
        tee!(log, "Using persisted payer: {}", kp.pubkey());
        tee!(log, "(saved at {})", PAYER_PATH);
        kp
    } else {
        let kp = Keypair::new();
        write_keypair_file(&kp, &payer_path)
            .map_err(|e| anyhow::anyhow!("failed to write payer keypair: {e}"))?;
        tee!(log, "Generated new payer:   {}", kp.pubkey());
        tee!(log, "(saved at {})", PAYER_PATH);
        kp
    };
    tee!(log, "");

    let agent = Keypair::new();
    tee!(log, "  Generated agent:  {}", agent.pubkey());
    tee!(log, "");

    // Fund payer via transfer or airdrop
    if let Some(fund_path) = cli.fund_from {
        let funder = read_keypair_file(&fund_path)
            .map_err(|e| anyhow::anyhow!("failed to read --fund-from keypair: {e}"))?;
        tee!(
            log,
            "  Transferring {} SOL from {} …",
            FUND_LAMPORTS as f64 / LAMPORTS_PER_SOL as f64,
            funder.pubkey()
        );
        let ix = system_instruction::transfer(&funder.pubkey(), &payer.pubkey(), FUND_LAMPORTS);
        let bh = rpc.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(&[ix], Some(&funder.pubkey()), &[&funder], bh);
        rpc.send_and_confirm_transaction(&tx).await?;
        tee!(log, "  Transfer confirmed.");
    } else {
        tee!(
            log,
            "  Requesting devnet airdrop ({} SOL) …",
            FUND_LAMPORTS as f64 / LAMPORTS_PER_SOL as f64
        );
        let sig = rpc.request_airdrop(&payer.pubkey(), FUND_LAMPORTS).await?;
        rpc.confirm_transaction_with_spinner(
            &sig,
            &rpc.get_latest_blockhash().await?,
            CommitmentConfig::confirmed(),
        )
        .await?;
        tee!(log, "  Airdrop confirmed.");
    }

    let balance = rpc.get_balance(&payer.pubkey()).await?;
    tee!(log, "  Payer balance: {} lamports", balance);
    tee!(log, "");

    tee!(
        log,
        "  Running TX1–TX4 (create governed spending account) …"
    );
    let account = create_governed_spending_account(
        &rpc,
        &payer,
        agent.pubkey(),
        payer.pubkey(), // governing authority = payer for demo
        SpendingTierConfig {
            max_auto_approve_lamports: LIMIT_LAMPORTS,
            period: SpendingPeriod::Day,
            mint: Pubkey::default(),
        },
    )
    .await
    .map_err(|e| anyhow::anyhow!("setup failed: {e}"))?;

    tee!(log, "  TX1–TX4 confirmed.");
    tee!(log, "");

    // Fund the vault so Attack 1 has a meaningful balance to exceed
    tee!(log, "  Funding vault with {} lamports …", VAULT_FUND);
    let fund_ix = system_instruction::transfer(&payer.pubkey(), &account.vault_pda, VAULT_FUND);
    let bh = rpc.get_latest_blockhash().await?;
    let fund_tx =
        Transaction::new_signed_with_payer(&[fund_ix], Some(&payer.pubkey()), &[&payer], bh);
    rpc.send_and_confirm_transaction(&fund_tx).await?;
    tee!(log, "  Vault funded.");

    // Fund agent with minimal SOL for tx fees. Without this, simulate-attack transactions
    // fail at the fee-payer level ("no record of a prior credit") before the Squads program
    // runs its own validation, masking the real on-chain rejection reason.
    tee!(
        log,
        "  Funding agent with {} lamports (tx fees) …",
        AGENT_FUND
    );
    let agent_fund_ix = system_instruction::transfer(&payer.pubkey(), &agent.pubkey(), AGENT_FUND);
    let bh = rpc.get_latest_blockhash().await?;
    let agent_fund_tx =
        Transaction::new_signed_with_payer(&[agent_fund_ix], Some(&payer.pubkey()), &[&payer], bh);
    rpc.send_and_confirm_transaction(&agent_fund_tx).await?;
    tee!(log, "  Agent funded.");
    tee!(log, "");

    // Save agent keypair to temp file so cerberus simulate-attack can load it
    let agent_path = std::env::temp_dir().join("cerberus_demo_agent.json");
    write_keypair_file(&agent, &agent_path)
        .map_err(|e| anyhow::anyhow!("failed to write agent keypair: {e}"))?;

    tee!(log, "  ✓ Setup complete");
    tee!(log, "");
    tee!(log, "  Payer:              {}", payer.pubkey());
    tee!(log, "  Multisig PDA:       {}", account.multisig_pda);
    tee!(log, "  Vault PDA:          {}", account.vault_pda);
    tee!(log, "  Spending Limit PDA: {}", account.spending_limit_pda);
    tee!(log, "  Agent keypair:      {}", agent_path.display());
    tee!(log, "");
    tee!(log, "  Run the attack simulation:");
    tee!(log, "");
    tee!(
        log,
        "  cargo run --bin cerberus -- simulate-attack {} \\",
        account.multisig_pda
    );
    tee!(log, "    --agent-keypair {} \\", agent_path.display());
    tee!(
        log,
        "    --spending-limit-pda {}",
        account.spending_limit_pda
    );

    Ok(())
}

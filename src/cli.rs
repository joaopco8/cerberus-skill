//! CLI argument definitions for the `cerberus` binary.
//!
//! Parsed by [`clap`] via its derive macro. The top-level [`Cli`] struct
//! holds global flags; [`Commands`] contains the three subcommands.

use clap::{Parser, Subcommand};

/// cerberus — on-chain governed spending limits for AI agent wallets
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Solana RPC endpoint
    #[arg(long, global = true, default_value = "https://api.devnet.solana.com")]
    pub rpc_url: String,

    /// Path to the payer / governing authority keypair
    #[arg(long, global = true, default_value = "~/.config/solana/id.json")]
    pub keypair: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new governed spending account for an agent wallet (TX1–TX4)
    Init {
        /// The agent wallet's public key (base58)
        #[arg(long)]
        agent_wallet: String,

        /// Max amount auto-approved per period, in lamports (native SOL) or token base units
        #[arg(long)]
        max_auto_approve: u64,

        /// Token mint (omit or pass default address for native SOL)
        #[arg(long)]
        mint: Option<String>,
    },

    /// Check the current lock status of a multisig PDA
    Status {
        /// The multisig PDA address (base58)
        multisig_pda: String,
    },

    /// Complete an interrupted TX1–TX4 setup (idempotent, safe on already-locked multisig)
    Recover {
        /// The multisig PDA address (base58)
        multisig_pda: String,

        /// Expected threshold after recovery
        #[arg(long, default_value = "1")]
        expected_threshold: u16,
    },
}

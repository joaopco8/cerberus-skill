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

    /// Attempt 3 real attack vectors against a governed account and prove each is
    /// rejected on-chain by the Squads program, not by application logic.
    ///
    /// Every attack submits a real transaction to devnet. Results are reported
    /// honestly — if an attack succeeds, it is flagged as a security finding.
    SimulateAttack {
        /// The multisig PDA to attack
        multisig_pda: String,

        /// Path to the AGENT wallet's keypair — the compromised key being simulated.
        /// This is NOT the governing authority; it is the key an attacker is assumed
        /// to have obtained (e.g., stolen from the agent's runtime environment).
        #[arg(long)]
        agent_keypair: String,

        /// The SpendingLimit PDA for this multisig. Required for Attack 1 (overspend).
        /// Omit to run only Attacks 2 and 3.
        #[arg(long)]
        spending_limit_pda: Option<String>,
    },
}

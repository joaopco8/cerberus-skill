//! # cerberus-skill
//!
//! On-chain governed spending limits for AI agent wallets on Solana,
//! built on [Squads Protocol v4](https://squads.so).
//!
//! ## The problem
//!
//! Database-stored spending limits are a promise, not a law. Any bug,
//! prompt injection, or compromised key can rewrite them at runtime.
//! Cerberus replaces them with cryptographically-enforced, on-chain rules
//! that require multisig approval to change.
//!
//! ## Architecture
//!
//! ```text
//! AI Agent Wallet
//!       │
//!       ▼
//! SpendingLimit PDA (Squads v4)
//!   ├── amount:   hard lamport cap per reset period
//!   ├── period:   Day / Week / Month / OneTime
//!   └── members:  which wallets may use this limit
//!       │
//!       ▼
//! Multisig PDA ──── config_authority: Pubkey::default() (locked)
//!   └── members: [governing_authority (Vote), agent_wallet (Execute)]
//! ```
//!
//! ## Setup flow (TX1–TX4)
//!
//! ```text
//! TX1  multisig_create_v2          configAuthority = payer (temporary)
//! TX2  multisig_add_spending_limit attach per-period cap
//! TX3  multisig_change_threshold   confirm governance threshold
//! TX4  multisig_set_config_authority  configAuthority → Pubkey::default()
//! ```
//!
//! After TX4 the multisig is permanently autonomous — no one can change
//! configuration without a multisig vote from `governing_authority`.
//!
//! ## Tiers
//!
//! | Tier | Amount | Action |
//! |------|--------|--------|
//! | 1 | ≤ limit | Auto-approved by on-chain spending limit |
//! | 2 | 1–5× limit | Extra verification required off-chain |
//! | 3 | > 5× limit | Human multisig proposal + vote |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use cerberus_skill::{
//!     create_governed_spending_account,
//!     spending_account::{SpendingTierConfig, SpendingPeriod},
//! };
//! use solana_client::nonblocking::rpc_client::RpcClient;
//! use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), cerberus_skill::error::CerberusError> {
//! let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());
//! let payer = Keypair::new();
//!
//! let account = create_governed_spending_account(
//!     &rpc,
//!     &payer,
//!     Keypair::new().pubkey(), // agent wallet
//!     Keypair::new().pubkey(), // governing authority
//!     SpendingTierConfig {
//!         max_auto_approve_lamports: 10_000_000,
//!         period: SpendingPeriod::Day,
//!         mint: Pubkey::default(), // native SOL
//!     },
//! ).await?;
//!
//! println!("multisig: {}", account.multisig_pda);
//! # Ok(())
//! # }
//! ```

pub mod attack_sim;
pub mod banner;
pub mod cli;
pub mod error;
pub mod lock;
pub mod recover;
pub mod spending_account;
pub mod verify;

pub use error::CerberusError;
pub use lock::LockState;
pub use spending_account::{
    create_governed_spending_account, SpendingAccount, SpendingPeriod, SpendingTierConfig,
};
pub use verify::assert_fully_locked;

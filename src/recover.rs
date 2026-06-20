//! Recovery from partial or interrupted Squads v4 setup.
//!
//! If TX1–TX4 is interrupted after TX1 or TX2 succeeds, the multisig exists
//! on-chain but is not yet locked. [`recover_partial_setup`] detects the
//! current state and completes the missing steps so the lock is applied.
//!
//! See `references/partial-failure-recovery.md` for the full decision tree.

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

use crate::error::CerberusError;
use crate::{lock, spending_account, verify};

// ── Public API ────────────────────────────────────────────────────────────────

/// Completes an interrupted TX1–TX4 setup for an existing multisig.
///
/// Reads the current on-chain [`lock::LockState`] and re-runs only the steps
/// that have not yet been applied:
///
/// - If `threshold != expected_threshold`: calls [`spending_account::raise_threshold`].
/// - If `config_authority != Pubkey::default()`: calls
///   [`spending_account::disable_config_authority`].
/// - Finally, asserts the lock with [`verify::assert_fully_locked`].
///
/// If the multisig is already fully locked, returns `Ok(())` immediately
/// without sending any transactions.
///
/// # Parameters
///
/// - `rpc`: Async Solana RPC client.
/// - `payer`: Keypair that is still the active `configAuthority` on-chain
///   (the same keypair used as `payer` in the original setup).
/// - `multisig_pda`: The multisig PDA address returned from TX1.
/// - `expected_threshold`: The intended governance threshold.
///
/// # Errors
///
/// - [`CerberusError::RpcError`] on any network failure.
/// - [`CerberusError::DeserializationError`] if the multisig account is malformed.
/// - [`CerberusError::LockVerificationFailed`] if the final lock assertion fails.
///
/// # Example
///
/// ```rust,no_run
/// use cerberus_skill::recover::recover_partial_setup;
/// use solana_client::nonblocking::rpc_client::RpcClient;
/// use solana_sdk::{pubkey::Pubkey, signature::Keypair};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), cerberus_skill::error::CerberusError> {
/// let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());
/// let payer = Keypair::new();
/// let multisig_pda: Pubkey = "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse().unwrap();
///
/// recover_partial_setup(&rpc, &payer, &multisig_pda, 1).await?;
/// println!("recovery complete — multisig is now fully locked");
/// # Ok(())
/// # }
/// ```
pub async fn recover_partial_setup(
    rpc: &RpcClient,
    payer: &Keypair,
    multisig_pda: &Pubkey,
    expected_threshold: u16,
) -> Result<(), CerberusError> {
    let state = lock::get_lock_state(rpc, multisig_pda).await?;

    if state.is_fully_locked(expected_threshold) {
        return Ok(());
    }

    // TX3 may not have run, or was run with the wrong threshold.
    if state.threshold != expected_threshold {
        spending_account::raise_threshold(rpc, payer, multisig_pda, expected_threshold).await?;
    }

    // TX4 may not have run — configAuthority is still the bootstrap payer.
    if state.config_authority != Pubkey::default() {
        spending_account::disable_config_authority(rpc, payer, multisig_pda).await?;
    }

    // Confirm the lock is fully applied.
    verify::assert_fully_locked(rpc, multisig_pda, expected_threshold).await
}

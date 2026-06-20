//! On-chain lock verification for Squads v4 multisigs.
//!
//! [`assert_fully_locked`] is the primary entry point. It reads the on-chain
//! multisig state and returns an error if the lock is not complete, making it
//! suitable as a post-condition check after the TX1–TX4 setup.
//!
//! AI agents can call this at startup and periodically during long-running
//! sessions to detect unauthorized configuration changes.

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::error::CerberusError;
use crate::lock;

// ── Public API ────────────────────────────────────────────────────────────────

/// Asserts that a Squads v4 multisig is fully locked after the TX1–TX4 setup.
///
/// A multisig is "fully locked" when:
/// 1. `config_authority == Pubkey::default()` — TX4 ran successfully and no
///    party can change configuration without a multisig vote.
/// 2. `threshold == expected_threshold` — TX3 set the intended governance
///    threshold.
///
/// # Errors
///
/// - [`CerberusError::RpcError`] if the account cannot be fetched.
/// - [`CerberusError::DeserializationError`] if the account data is malformed.
/// - [`CerberusError::LockVerificationFailed`] if any assertion fails, with
///   the `field`, `expected`, and `actual` values for diagnosis.
///
/// # Example
///
/// ```rust,no_run
/// use cerberus_skill::verify::assert_fully_locked;
/// use solana_client::nonblocking::rpc_client::RpcClient;
/// use solana_sdk::pubkey::Pubkey;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), cerberus_skill::error::CerberusError> {
/// let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());
/// let multisig_pda: Pubkey = "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse().unwrap();
///
/// // Panics (returns Err) if TX4 has not yet run or threshold is wrong.
/// assert_fully_locked(&rpc, &multisig_pda, 1).await?;
/// println!("multisig is fully locked");
/// # Ok(())
/// # }
/// ```
pub async fn assert_fully_locked(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
    expected_threshold: u16,
) -> Result<(), CerberusError> {
    let state = lock::get_lock_state(rpc, multisig_pda).await?;

    if state.config_authority != Pubkey::default() {
        return Err(CerberusError::LockVerificationFailed {
            field: "config_authority".to_string(),
            expected: "Pubkey::default() (disabled)".to_string(),
            actual: state.config_authority.to_string(),
        });
    }

    if state.threshold != expected_threshold {
        return Err(CerberusError::LockVerificationFailed {
            field: "threshold".to_string(),
            expected: expected_threshold.to_string(),
            actual: state.threshold.to_string(),
        });
    }

    Ok(())
}

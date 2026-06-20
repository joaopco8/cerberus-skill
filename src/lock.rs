//! Inspect the lock state of a Squads v4 multisig.
//!
//! [`get_lock_state`] reads the on-chain [`Multisig`] account and returns a
//! [`LockState`] snapshot. Use [`LockState::is_fully_locked`] to confirm that
//! TX4 completed successfully and the `configAuthority` is permanently disabled.

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use squads_multisig::{anchor_lang::AccountDeserialize, state::Multisig};

use crate::error::CerberusError;

// ── Public types ──────────────────────────────────────────────────────────────

/// Snapshot of the governance lock state for a Squads v4 multisig.
#[derive(Debug, Clone)]
pub struct LockState {
    /// The current `configAuthority` on-chain.
    ///
    /// [`Pubkey::default()`] (all zeros) means the multisig is autonomous:
    /// no one can change configuration without a multisig vote. Any other
    /// value indicates the multisig is still "controlled" and the authority
    /// can make unilateral changes.
    pub config_authority: Pubkey,
    /// Current approval threshold (k in k-of-n).
    pub threshold: u16,
}

impl LockState {
    /// Returns `true` iff the multisig is fully locked:
    /// - `configAuthority == Pubkey::default()` (TX4 was applied), AND
    /// - `threshold == expected_threshold` (TX3 was applied).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cerberus_skill::lock::{get_lock_state, LockState};
    /// use solana_client::nonblocking::rpc_client::RpcClient;
    /// use solana_sdk::pubkey::Pubkey;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cerberus_skill::error::CerberusError> {
    /// let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());
    /// let multisig_pda: Pubkey = "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse().unwrap();
    ///
    /// let state = get_lock_state(&rpc, &multisig_pda).await?;
    /// assert!(state.is_fully_locked(1), "TX4 must have run: config_authority should be default");
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_fully_locked(&self, expected_threshold: u16) -> bool {
        self.config_authority == Pubkey::default() && self.threshold == expected_threshold
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetches the on-chain [`Multisig`] account and returns its lock state.
///
/// This is a lightweight read-only call — no transactions are sent.
///
/// # Errors
///
/// - [`CerberusError::RpcError`] if the account cannot be fetched.
/// - [`CerberusError::DeserializationError`] if the account data is malformed.
///
/// # Example
///
/// ```rust,no_run
/// use cerberus_skill::lock::get_lock_state;
/// use solana_client::nonblocking::rpc_client::RpcClient;
/// use solana_sdk::pubkey::Pubkey;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), cerberus_skill::error::CerberusError> {
/// let rpc = RpcClient::new("https://api.devnet.solana.com".to_string());
/// let multisig_pda: Pubkey = "JCxMQNWf2VbxM9dA2ut2Pu767Et447giMgSqufdegpKr".parse().unwrap();
///
/// let state = get_lock_state(&rpc, &multisig_pda).await?;
/// println!("config_authority: {}", state.config_authority);
/// println!("threshold:        {}", state.threshold);
/// # Ok(())
/// # }
/// ```
pub async fn get_lock_state(
    rpc: &RpcClient,
    multisig_pda: &Pubkey,
) -> Result<LockState, CerberusError> {
    let account = rpc.get_account(multisig_pda).await?;
    let multisig = Multisig::try_deserialize(&mut account.data.as_slice()).map_err(|e| {
        CerberusError::DeserializationError {
            address: multisig_pda.to_string(),
            message: e.to_string(),
        }
    })?;

    Ok(LockState {
        config_authority: multisig.config_authority,
        threshold: multisig.threshold,
    })
}

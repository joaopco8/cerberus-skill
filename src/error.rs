//! Unified error type for cerberus-skill.
//!
//! [`CerberusError`] is the single source of truth for every failure condition
//! this crate can produce. All public functions return `Result<_, CerberusError>`.

/// All errors that cerberus-skill functions can produce.
///
/// Use [`std::error::Error::source`] to inspect the underlying cause for
/// variants that wrap external errors.
#[derive(Debug, thiserror::Error)]
pub enum CerberusError {
    /// A Solana RPC call failed. Wraps [`solana_client::client_error::ClientError`].
    ///
    /// Boxed to keep `CerberusError` within a reasonable stack size.
    #[error("RPC error: {0}")]
    RpcError(Box<solana_client::client_error::ClientError>),

    /// On-chain state does not match the expected lock configuration.
    ///
    /// Indicates tampering or a concurrent modification between setup and verification.
    #[error("lock verification failed — field `{field}`: expected `{expected}`, got `{actual}`")]
    LockVerificationFailed {
        /// The name of the mismatched field (e.g. `"amount"`, `"threshold"`).
        field: String,
        /// The value that was expected.
        expected: String,
        /// The value actually found on-chain.
        actual: String,
    },

    /// Setup completed only partially before a failure; on-chain state is inconsistent.
    ///
    /// Call [`crate::recover::detect_partial_setup`] to determine the current stage,
    /// then [`crate::recover::resume_setup`] or [`crate::recover::cleanup_partial`].
    #[error(
        "partial setup detected for multisig {multisig_pda} at stage `{stage}`; \
         call recover::resume_setup to complete or recover::cleanup_partial to abort"
    )]
    PartialSetupDetected {
        /// The multisig PDA address (base58) where the partial state exists.
        multisig_pda: String,
        /// How far setup progressed before the failure.
        stage: SetupStage,
    },

    /// The payer wallet does not hold enough lamports for setup fees and rent.
    #[error(
        "insufficient funds: wallet has {available_lamports} lamports, \
         needs at least {required_lamports} lamports (~{:.4} SOL)",
        *required_lamports as f64 / 1_000_000_000.0
    )]
    InsufficientFunds {
        /// Lamports currently in the payer account.
        available_lamports: u64,
        /// Minimum lamports needed to proceed.
        required_lamports: u64,
    },

    /// The spending limit amount or period is not valid.
    #[error("invalid spending limit: {reason}")]
    InvalidSpendingLimitAmount {
        /// Human-readable explanation of why the value was rejected.
        reason: String,
    },

    /// Failed to deserialize a Squads account from RPC data.
    #[error("account deserialization failed for {address}: {message}")]
    DeserializationError {
        /// Base58 address of the account that could not be deserialized.
        address: String,
        /// Error message from the deserializer.
        message: String,
    },

    /// A transaction simulation returned errors before it was sent.
    #[error("transaction simulation failed:\n{logs}")]
    SimulationFailed {
        /// Program logs from the failed simulation.
        logs: String,
    },
}

/// How far through the two-step Squads setup the process got before failure.
///
/// Used in [`CerberusError::PartialSetupDetected`] to guide recovery logic.
/// The variants are ordered: each implies the previous step succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupStage {
    /// Multisig PDA was created but the spending limit was not yet attached.
    MultisigCreated,
    /// Spending limit was created but member permissions were not yet configured.
    SpendingLimitCreated,
    /// Members were configured but the lock was not finalized.
    MembersConfigured,
}

impl From<solana_client::client_error::ClientError> for CerberusError {
    fn from(e: solana_client::client_error::ClientError) -> Self {
        CerberusError::RpcError(Box::new(e))
    }
}

impl std::fmt::Display for SetupStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MultisigCreated => write!(f, "multisig-created"),
            Self::SpendingLimitCreated => write!(f, "spending-limit-created"),
            Self::MembersConfigured => write!(f, "members-configured"),
        }
    }
}

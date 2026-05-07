use thiserror::Error;

#[derive(Debug, Error)]
pub enum OracleError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("entropy fetch failed for source {provider}: {reason}")]
    Entropy { provider: &'static str, reason: String },

    #[error("epoch phase mismatch: expected {expected}, got {actual}")]
    PhaseMismatch { expected: String, actual: String },

    #[error("VRF error: {0}")]
    Vrf(String),

    #[error("transaction error: {0}")]
    Transaction(String),
}

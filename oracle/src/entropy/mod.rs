//! Entropy sources for Random Dungeon.
//!
//! See `docs/entropy.md` for the spec. This module owns the
//! `EntropySource` trait, fetcher implementations, canonicalization,
//! manifest construction, and seed derivation.
//!
//! The contract this module exposes to the rest of the oracle is
//! `build_entropy_bundle(epoch) -> EntropyBundle`. Everything else is
//! internal.

pub mod btc;
pub mod canonical;
pub mod drand;
pub mod manifest;
pub mod nws;
pub mod seed;
pub mod usgs;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Domain-separation tag. Bumping the version produces different seeds
/// for the same inputs; do this only on a breaking spec change.
pub const DOMAIN_TAG: &[u8] = b"random-dungeon/entropy/v1";

/// One source's raw response, with metadata for archival and audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSample {
    /// Source identifier: "btc" | "drand" | "nws" | "usgs".
    pub source: &'static str,
    /// Wall-clock time of fetch, ms since unix epoch.
    pub fetched_at_ms: i64,
    /// Endpoint URL we hit (for the archive record).
    pub endpoint: String,
    /// Raw response bytes — the unmodified API payload.
    pub payload: Vec<u8>,
}

/// One source's canonicalized byte string. This is what gets hashed
/// into the manifest. See `docs/entropy.md` for layouts.
#[derive(Debug, Clone)]
pub struct CanonicalSample {
    pub source: &'static str,
    pub bytes: Vec<u8>,
}

/// Final per-epoch entropy artifact handed to the VRF stage.
#[derive(Debug, Clone)]
pub struct EntropyBundle {
    pub epoch: u64,
    /// Timestamp (ms since unix epoch) baked into the manifest hash.
    pub fetched_at_ms: i64,
    /// SHA256 of the manifest. Goes on-chain.
    pub manifest_hash: [u8; 32],
    /// The seed s_t. Input to the VRF.
    pub seed: [u8; 32],
    /// Raw responses, kept for archival.
    pub raw_samples: Vec<RawSample>,
    /// Canonical bytes per source, kept for audit.
    pub canonical_samples: Vec<CanonicalSample>,
}

/// Trait every entropy source implements. Fetcher returns raw bytes;
/// canonicalization is a separate concern.
#[async_trait::async_trait]
pub trait EntropySource: Send + Sync {
    fn name(&self) -> &'static str;

    /// Fetch the raw response. On any failure, return Err — the
    /// MVP fail-loud policy aborts the epoch on a single source error.
    async fn fetch(&self) -> Result<RawSample>;

    /// Canonicalize a previously-fetched raw response. Pure function:
    /// same RawSample → same CanonicalSample, every time, on every
    /// machine. This is what the reproducibility test pins down.
    fn canonicalize(&self, raw: &RawSample) -> Result<CanonicalSample>;
}

/// Fetch all sources in parallel; on any error, abort the whole epoch.
/// Per-MVP policy: fail loud rather than silently degrade.
pub async fn gather_all(sources: &[Box<dyn EntropySource>]) -> Result<Vec<RawSample>> {
    use futures::future::try_join_all;
    let futures = sources.iter().map(|s| s.fetch());
    let samples = try_join_all(futures).await?;
    Ok(samples)
}

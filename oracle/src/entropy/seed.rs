//! Seed derivation and the public entry point.
//!
//! This module wires the entropy sources together. Everything else in
//! the oracle imports just `build_entropy_bundle`.

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::path::Path;

use super::manifest;
use super::{
    btc::BtcSource, drand::DrandSource, gather_all, nws::NwsSource, usgs::UsgsSource,
    CanonicalSample, EntropyBundle, EntropySource, RawSample, DOMAIN_TAG,
};

/// Build the four MVP sources with a shared HTTP client. Timeout is
/// generous so a slow source doesn't silently fail; on real failures
/// we want clear errors, not timeouts.
pub fn default_sources() -> Result<Vec<Box<dyn EntropySource>>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("seed: build http client")?;
    Ok(vec![
        Box::new(BtcSource::new(client.clone())),
        Box::new(DrandSource::new(client.clone())),
        Box::new(NwsSource::new(client.clone())),
        Box::new(UsgsSource::new(client)),
    ])
}

/// End-to-end entropy build: fetch all sources, canonicalize, build
/// manifest, derive seed. On any source failure, returns Err and the
/// caller is expected to abort the epoch.
pub async fn build_entropy_bundle(epoch: u64) -> Result<EntropyBundle> {
    let sources = default_sources()?;
    let raw_samples = gather_all(&sources).await?;
    finalize(epoch, sources, raw_samples)
}

/// Pure portion of the pipeline: given raw samples, produce the
/// canonical samples, manifest, and seed. Split out so reproducibility
/// tests can run without network access.
pub fn finalize(
    epoch: u64,
    sources: Vec<Box<dyn EntropySource>>,
    raw_samples: Vec<RawSample>,
) -> Result<EntropyBundle> {
    // Canonicalize each raw sample using its corresponding source.
    // We pair them up by source name to be defensive against ordering
    // changes in `gather_all`.
    let mut canonical_samples: Vec<CanonicalSample> = Vec::with_capacity(sources.len());
    for raw in &raw_samples {
        let src = sources
            .iter()
            .find(|s| s.name() == raw.source)
            .ok_or_else(|| anyhow::anyhow!("seed: no source for {:?}", raw.source))?;
        canonical_samples.push(src.canonicalize(raw)?);
    }

    let fetched_at_ms = Utc::now().timestamp_millis();
    let m = manifest::build(epoch, fetched_at_ms, &canonical_samples)?;
    let manifest_hash = manifest::hash(&m)?;
    let seed = derive_seed(&manifest_hash);

    Ok(EntropyBundle {
        epoch,
        manifest_hash,
        seed,
        raw_samples,
        canonical_samples,
    })
}

/// `s_t = SHA256(DOMAIN_TAG || "seed" || manifest_hash)`
pub fn derive_seed(manifest_hash: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN_TAG);
    h.update(b"seed");
    h.update(manifest_hash);
    h.finalize().into()
}

/// Write raw samples + manifest to the local archive at
/// `oracle/archive/<epoch>/`.
pub fn archive(bundle: &EntropyBundle, archive_root: &Path) -> Result<()> {
    let dir = archive_root.join(bundle.epoch.to_string());
    std::fs::create_dir_all(&dir).context("archive: create epoch dir")?;

    for raw in &bundle.raw_samples {
        let path = dir.join(format!("{}.json", raw.source));
        std::fs::write(&path, &raw.payload)
            .with_context(|| format!("archive: write {}", path.display()))?;
    }

    // Structured manifest record (for human / auditor consumption).
    let manifest_record = serde_json::json!({
        "epoch": bundle.epoch,
        "manifest_hash": hex::encode(bundle.manifest_hash),
        "seed": hex::encode(bundle.seed),
        "sources": bundle.raw_samples.iter().map(|r| serde_json::json!({
            "name": r.source,
            "endpoint": r.endpoint,
            "fetched_at_ms": r.fetched_at_ms,
        })).collect::<Vec<_>>(),
    });
    let path = dir.join("manifest.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest_record)?)
        .with_context(|| format!("archive: write {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_seed_is_deterministic() {
        let h = [7u8; 32];
        assert_eq!(derive_seed(&h), derive_seed(&h));
    }

    #[test]
    fn derive_seed_changes_with_input() {
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        assert_ne!(derive_seed(&h1), derive_seed(&h2));
    }
}

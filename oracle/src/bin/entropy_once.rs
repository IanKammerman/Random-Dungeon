//! Fetch one entropy bundle for the given epoch and write it to
//! `oracle/archive/<epoch>/`. Used to populate archive snapshots that
//! the static visualizer in `web/` reads.
//!
//! Usage: `cargo run -p oracle --bin entropy_once <EPOCH_ID>`
//!
//! On-disk layout per epoch:
//!   btc.json     — raw blockchain.info response
//!   drand.json   — raw drand response
//!   nws.json     — newline-delimited per-station NWS observations
//!   usgs.json    — raw USGS GeoJSON feed
//!   manifest.json — enriched manifest record
//!
//! The manifest record carries enough information to re-derive
//! `manifest_hash` and `seed` without re-canonicalizing the raw JSON:
//! it includes per-source canonical hashes, canonical byte lengths,
//! and the `fetched_at_ms` used in the manifest. The visualizer's
//! "Recompute manifest_hash" button uses these fields directly.
//!
//! `seed.rs::build_entropy_bundle` does not expose the
//! `fetched_at_ms` it stamped into the in-memory manifest, so this
//! binary rebuilds the manifest with its own timestamp and writes
//! both the resulting hash and that timestamp to disk. The
//! reproducibility check `manifest_hash = SHA256(serialize(manifest))`
//! is what matters; the value of `fetched_at_ms` is only required to
//! be the same one used when building the hash.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use oracle::entropy::seed::build_entropy_bundle;

const DOMAIN_TAG: &[u8] = b"random-dungeon/entropy/v1";

#[tokio::main]
async fn main() -> Result<()> {
    let epoch: u64 = std::env::args()
        .nth(1)
        .context("usage: entropy_once <EPOCH_ID>")?
        .parse()
        .context("EPOCH_ID must be a u64")?;

    let bundle = build_entropy_bundle(epoch).await?;

    let archive_root: PathBuf = std::env::var("ORACLE_ARCHIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("oracle/archive"));
    let dir = archive_root.join(epoch.to_string());
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create archive dir {}", dir.display()))?;

    for raw in &bundle.raw_samples {
        let path = dir.join(format!("{}.json", raw.source));
        std::fs::write(&path, &raw.payload)
            .with_context(|| format!("write {}", path.display()))?;
    }

    // Pair each canonical sample with its hash and length.
    let mut canonical_by_source = std::collections::HashMap::new();
    for canon in &bundle.canonical_samples {
        let hash: [u8; 32] = Sha256::digest(&canon.bytes).into();
        canonical_by_source.insert(canon.source.to_string(), (hash, canon.bytes.len()));
    }

    // Build the source records sorted by name (matches manifest layout).
    let mut sources_json: Vec<serde_json::Value> = bundle
        .raw_samples
        .iter()
        .map(|r| {
            let (hash, len) = canonical_by_source
                .get(r.source)
                .copied()
                .unwrap_or(([0u8; 32], 0));
            serde_json::json!({
                "name": r.source,
                "endpoint": r.endpoint,
                "fetched_at_ms": r.fetched_at_ms,
                "canonical_hash": hex::encode(hash),
                "canonical_len": len,
            })
        })
        .collect();
    sources_json.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });

    // Rebuild the manifest with our own timestamp so we can record both
    // the hash and the timestamp used to compute it.
    let manifest_fetched_at_ms = chrono::Utc::now().timestamp_millis();
    let manifest_bytes =
        build_manifest_bytes(epoch, manifest_fetched_at_ms, &sources_json)?;
    let manifest_hash: [u8; 32] = Sha256::digest(&manifest_bytes).into();
    let seed = derive_seed(&manifest_hash);

    let manifest_record = serde_json::json!({
        "epoch": epoch,
        "manifest_fetched_at_ms": manifest_fetched_at_ms,
        "manifest_hash": hex::encode(manifest_hash),
        "seed": hex::encode(seed),
        "domain_tag": "random-dungeon/entropy/v1",
        "sources": sources_json,
    });

    let manifest_path = dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_record)?,
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;

    eprintln!(
        "epoch {} written to {} (manifest_hash={})",
        epoch,
        dir.display(),
        hex::encode(manifest_hash)
    );

    Ok(())
}

fn build_manifest_bytes(
    epoch: u64,
    fetched_at_ms: i64,
    sources_json: &[serde_json::Value],
) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(128 + sources_json.len() * 64);
    buf.extend_from_slice(DOMAIN_TAG);
    buf.extend_from_slice(b"manifest");
    buf.extend_from_slice(&epoch.to_be_bytes());
    buf.extend_from_slice(&fetched_at_ms.to_be_bytes());
    buf.extend_from_slice(&(sources_json.len() as u32).to_be_bytes());
    for rec in sources_json {
        let name = rec["name"].as_str().context("manifest record name")?;
        let canonical_hash_hex = rec["canonical_hash"]
            .as_str()
            .context("manifest record canonical_hash")?;
        let canonical_hash =
            hex::decode(canonical_hash_hex).context("decode canonical_hash hex")?;
        if canonical_hash.len() != 32 {
            anyhow::bail!("canonical_hash is not 32 bytes for {}", name);
        }
        buf.extend_from_slice(&(name.len() as u16).to_be_bytes());
        buf.extend_from_slice(name.as_bytes());
        buf.extend_from_slice(&canonical_hash);
    }
    Ok(buf)
}

fn derive_seed(manifest_hash: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN_TAG);
    h.update(b"seed");
    h.update(manifest_hash);
    h.finalize().into()
}

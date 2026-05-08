//! Manifest construction.
//!
//! The manifest is the on-chain commitment binding `epoch_id` to the
//! exact set of canonical inputs that produced `s_t`. See
//! `docs/entropy.md` for the byte layout.

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};

use super::canonical as c;
use super::{CanonicalSample, DOMAIN_TAG};

/// One per-source manifest record.
pub struct ManifestRecord {
    pub source: String,
    pub canonical_hash: [u8; 32],
}

pub struct Manifest {
    pub epoch: u64,
    pub fetched_at_ms: i64,
    pub records: Vec<ManifestRecord>,
}

/// Build the manifest from canonical samples + epoch metadata.
/// Records are sorted ascending by source name.
pub fn build(
    epoch: u64,
    fetched_at_ms: i64,
    canonical_samples: &[CanonicalSample],
) -> Result<Manifest> {
    if canonical_samples.is_empty() {
        return Err(anyhow!("manifest: cannot build with zero sources"));
    }
    let mut records: Vec<ManifestRecord> = canonical_samples
        .iter()
        .map(|s| ManifestRecord {
            source: s.source.to_string(),
            canonical_hash: sha256(&s.bytes),
        })
        .collect();
    records.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(Manifest {
        epoch,
        fetched_at_ms,
        records,
    })
}

/// Serialize the manifest to its canonical byte form. The hash of this
/// byte string is what goes on-chain.
pub fn serialize(m: &Manifest) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(128 + m.records.len() * 64);
    buf.extend_from_slice(DOMAIN_TAG);
    buf.extend_from_slice(b"manifest");
    c::write_u64_be(&mut buf, m.epoch);
    c::write_i64_be(&mut buf, m.fetched_at_ms);
    let count: u32 = m
        .records
        .len()
        .try_into()
        .map_err(|_| anyhow!("manifest: too many records"))?;
    c::write_u32_be(&mut buf, count);
    for rec in &m.records {
        c::write_lp_str(&mut buf, &rec.source)?;
        buf.extend_from_slice(&rec.canonical_hash);
    }
    Ok(buf)
}

pub fn hash(m: &Manifest) -> Result<[u8; 32]> {
    Ok(sha256(&serialize(m)?))
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_sample(source: &'static str, byte: u8) -> CanonicalSample {
        CanonicalSample {
            source,
            bytes: vec![byte; 32],
        }
    }

    #[test]
    fn records_sorted_by_source() {
        let samples = vec![
            fake_sample("usgs", 1),
            fake_sample("btc", 2),
            fake_sample("drand", 3),
            fake_sample("nws", 4),
        ];
        let m = build(42, 1_700_000_000_000, &samples).unwrap();
        let names: Vec<_> = m.records.iter().map(|r| r.source.as_str()).collect();
        assert_eq!(names, vec!["btc", "drand", "nws", "usgs"]);
    }

    #[test]
    fn hash_is_deterministic() {
        let samples = vec![fake_sample("btc", 1), fake_sample("nws", 2)];
        let m1 = build(7, 12345, &samples).unwrap();
        let m2 = build(7, 12345, &samples).unwrap();
        assert_eq!(hash(&m1).unwrap(), hash(&m2).unwrap());
    }

    #[test]
    fn epoch_change_changes_hash() {
        let samples = vec![fake_sample("btc", 1)];
        let h1 = hash(&build(1, 0, &samples).unwrap()).unwrap();
        let h2 = hash(&build(2, 0, &samples).unwrap()).unwrap();
        assert_ne!(h1, h2);
    }
}

//! drand public randomness beacon (League of Entropy mainnet).
//!
//! Endpoint: <https://api.drand.sh/public/latest>
//!
//! Returns the current `(round, randomness, signature, previous_signature)`.
//! We canonicalize round, randomness, and signature; previous_signature is
//! redundant for our purposes.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use super::canonical as c;
use super::{CanonicalSample, EntropySource, RawSample};

pub const SOURCE: &str = "drand";
pub const ENDPOINT: &str = "https://api.drand.sh/public/latest";

pub struct DrandSource {
    client: Client,
}

impl DrandSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl EntropySource for DrandSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn fetch(&self) -> Result<RawSample> {
        let resp = self
            .client
            .get(ENDPOINT)
            .send()
            .await
            .context("drand: request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("drand: non-2xx status {}", resp.status()));
        }
        let payload = resp.bytes().await.context("drand: read body")?.to_vec();
        let _: DrandRound =
            serde_json::from_slice(&payload).context("drand: parse round JSON")?;
        Ok(RawSample {
            source: SOURCE,
            fetched_at_ms: Utc::now().timestamp_millis(),
            endpoint: ENDPOINT.to_string(),
            payload,
        })
    }

    fn canonicalize(&self, raw: &RawSample) -> Result<CanonicalSample> {
        let parsed: DrandRound =
            serde_json::from_slice(&raw.payload).context("drand: parse round JSON")?;

        let randomness = c::parse_hex_32(&parsed.randomness)?;
        let signature_bytes = c::parse_hex_var(&parsed.signature)?;

        let mut buf = c::open(SOURCE);
        c::write_u64_be(&mut buf, parsed.round);
        buf.extend_from_slice(&randomness);
        c::write_lp_bytes(&mut buf, &signature_bytes)?;

        // Note: signature verification (BLS over chain group key) is a
        // stretch goal. For MVP we trust the api.drand.sh endpoint.
        // TODO: add signature verification using the drand chain info
        // endpoint (`/info`) to fetch the public key.

        Ok(CanonicalSample {
            source: SOURCE,
            bytes: buf,
        })
    }
}

#[derive(Debug, Deserialize)]
struct DrandRound {
    round: u64,
    /// 64 hex chars = 32 bytes (SHA-256 of the BLS signature).
    randomness: String,
    /// BLS12-381 G1 signature, hex (96 hex chars = 48 bytes on mainnet).
    signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/drand/latest.json");

    #[test]
    fn canonicalize_is_deterministic() {
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: ENDPOINT.to_string(),
            payload: FIXTURE.as_bytes().to_vec(),
        };
        let src = DrandSource::new(Client::new());
        let a = src.canonicalize(&raw).unwrap();
        let b = src.canonicalize(&raw).unwrap();
        assert_eq!(a.bytes, b.bytes);
    }
}

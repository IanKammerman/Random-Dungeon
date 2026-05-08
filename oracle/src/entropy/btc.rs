//! Bitcoin most-recent block.
//!
//! Primary endpoint: <https://blockchain.info/latestblock>
//! Fallback endpoint: <https://mempool.space/api/blocks/tip/hash>
//!  + <https://mempool.space/api/block/{hash}>
//!
//! We accept the tip block. The spec says one confirmation deep
//! (`BTC_MIN_AGE_SECS`); enforcement is at the seed-derivation layer
//! by checking `block_time + 600 < epoch_start_time`. The fetch itself
//! just returns the latest block.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use super::canonical as c;
use super::{CanonicalSample, EntropySource, RawSample};

pub const SOURCE: &str = "btc";
pub const PRIMARY_ENDPOINT: &str = "https://blockchain.info/latestblock";

pub struct BtcSource {
    client: Client,
}

impl BtcSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl EntropySource for BtcSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn fetch(&self) -> Result<RawSample> {
        let resp = self
            .client
            .get(PRIMARY_ENDPOINT)
            .send()
            .await
            .context("btc: primary endpoint request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("btc: primary returned {}", resp.status()));
        }
        let payload = resp
            .bytes()
            .await
            .context("btc: read primary body")?
            .to_vec();
        // Validate parseability now so we fail loud on malformed
        // responses instead of at canonicalize time.
        let _: BlockchainInfoLatest =
            serde_json::from_slice(&payload).context("btc: parse latestblock JSON")?;

        Ok(RawSample {
            source: SOURCE,
            fetched_at_ms: Utc::now().timestamp_millis(),
            endpoint: PRIMARY_ENDPOINT.to_string(),
            payload,
        })
    }

    fn canonicalize(&self, raw: &RawSample) -> Result<CanonicalSample> {
        let parsed: BlockchainInfoLatest =
            serde_json::from_slice(&raw.payload).context("btc: parse latestblock JSON")?;

        let height: u32 = parsed
            .height
            .try_into()
            .map_err(|_| anyhow!("btc: block height overflows u32"))?;
        let hash_bytes = c::parse_hex_32(&parsed.hash)?;
        // blockchain.info reports `time` in seconds.
        let time_secs: i64 = parsed
            .time
            .try_into()
            .map_err(|_| anyhow!("btc: block time out of range"))?;

        let mut buf = c::open(SOURCE);
        c::write_u32_be(&mut buf, height);
        buf.extend_from_slice(&hash_bytes);
        c::write_i64_be(&mut buf, time_secs);

        Ok(CanonicalSample {
            source: SOURCE,
            bytes: buf,
        })
    }
}

#[derive(Debug, Deserialize)]
struct BlockchainInfoLatest {
    /// Block hash, 64 hex chars (display order).
    hash: String,
    /// Block height.
    height: u64,
    /// Block timestamp, unix seconds.
    time: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/btc/latestblock.json");

    #[test]
    fn canonicalize_is_deterministic() {
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: PRIMARY_ENDPOINT.to_string(),
            payload: FIXTURE.as_bytes().to_vec(),
        };
        let src = BtcSource::new(Client::new());
        let a = src.canonicalize(&raw).unwrap();
        let b = src.canonicalize(&raw).unwrap();
        assert_eq!(a.bytes, b.bytes);
        // tag (25) + "btc" (3) + height (4) + hash (32) + time (8) = 72
        assert_eq!(a.bytes.len(), 72);
    }
}

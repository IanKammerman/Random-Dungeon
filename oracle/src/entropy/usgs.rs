//! USGS earthquake feed (all events in the past hour).
//!
//! Endpoint: <https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson>
//! Auth: none.
//!
//! Canonicalization: see `docs/entropy.md`. Events are sorted ascending
//! by event id (lexicographic on UTF-8 bytes). Numeric fields are
//! fixed-point: magnitude × 1e6, lat/lon × 1e6 (microdegrees), depth in
//! millimeters.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use super::canonical as c;
use super::{CanonicalSample, EntropySource, RawSample};

pub const SOURCE: &str = "usgs";
pub const ENDPOINT: &str =
    "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson";

pub struct UsgsSource {
    client: Client,
}

impl UsgsSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl EntropySource for UsgsSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn fetch(&self) -> Result<RawSample> {
        let resp = self
            .client
            .get(ENDPOINT)
            .send()
            .await
            .context("usgs: request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("usgs: non-2xx status {}", resp.status()));
        }
        let payload = resp.bytes().await.context("usgs: read body")?.to_vec();
        Ok(RawSample {
            source: SOURCE,
            fetched_at_ms: Utc::now().timestamp_millis(),
            endpoint: ENDPOINT.to_string(),
            payload,
        })
    }

    fn canonicalize(&self, raw: &RawSample) -> Result<CanonicalSample> {
        let parsed: UsgsFeed =
            serde_json::from_slice(&raw.payload).context("usgs: parse GeoJSON")?;

        // Sort events by id ascending so order does not depend on the
        // server's response order.
        let mut events = parsed.features;
        events.sort_by(|a, b| a.id.cmp(&b.id));

        let mut buf = c::open(SOURCE);
        let count: u32 = events
            .len()
            .try_into()
            .map_err(|_| anyhow!("usgs: too many events"))?;
        c::write_u32_be(&mut buf, count);

        for ev in &events {
            // id (length-prefixed UTF-8)
            c::write_lp_str(&mut buf, &ev.id)?;

            // time, updated (ms since epoch)
            c::write_i64_be(&mut buf, ev.properties.time);
            c::write_i64_be(&mut buf, ev.properties.updated);

            // magnitude × 1_000_000
            let mag = c::float_to_fixed_i32(ev.properties.mag, 1_000_000.0);
            c::write_i32_be(&mut buf, mag);

            // geometry: [lon, lat, depth_km] — note the GeoJSON ordering.
            let (lon, lat, depth_km) = match ev.geometry.coordinates.as_slice() {
                [lon, lat, depth] => (Some(*lon), Some(*lat), Some(*depth)),
                _ => return Err(anyhow!("usgs: event {} has bad geometry", ev.id)),
            };
            c::write_i32_be(&mut buf, c::float_to_fixed_i32(lat, 1_000_000.0));
            c::write_i32_be(&mut buf, c::float_to_fixed_i32(lon, 1_000_000.0));
            // depth in km → millimeters: × 1_000_000
            c::write_i32_be(&mut buf, c::float_to_fixed_i32(depth_km, 1_000_000.0));
        }

        Ok(CanonicalSample {
            source: SOURCE,
            bytes: buf,
        })
    }
}

// Minimal GeoJSON shape we care about. Extra fields are ignored.

#[derive(Debug, Deserialize)]
struct UsgsFeed {
    features: Vec<UsgsFeature>,
}

#[derive(Debug, Deserialize)]
struct UsgsFeature {
    id: String,
    properties: UsgsProps,
    geometry: UsgsGeometry,
}

#[derive(Debug, Deserialize)]
struct UsgsProps {
    /// Event time, ms since epoch.
    time: i64,
    /// Last-update time, ms since epoch.
    updated: i64,
    /// Magnitude. Can be null for very small / preliminary events.
    mag: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct UsgsGeometry {
    /// `[longitude, latitude, depth_km]`.
    coordinates: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/usgs/all_hour.json");

    #[test]
    fn canonicalize_is_deterministic() {
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: ENDPOINT.to_string(),
            payload: FIXTURE.as_bytes().to_vec(),
        };
        let src = UsgsSource::new(Client::new());
        let a = src.canonicalize(&raw).unwrap();
        let b = src.canonicalize(&raw).unwrap();
        assert_eq!(a.bytes, b.bytes);
        // Sanity: domain tag prefix is present.
        assert!(a.bytes.starts_with(super::super::DOMAIN_TAG));
    }

    #[test]
    fn empty_feed_canonicalizes_to_count_zero() {
        let empty = br#"{"features":[]}"#.to_vec();
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: ENDPOINT.to_string(),
            payload: empty,
        };
        let src = UsgsSource::new(Client::new());
        let canon = src.canonicalize(&raw).unwrap();
        // tag (25) + "usgs" (4) + count (4) = 33 bytes
        assert_eq!(canon.bytes.len(), 33);
    }
}

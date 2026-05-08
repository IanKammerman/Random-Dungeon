//! NOAA National Weather Service observations.
//!
//! Endpoint per station:
//! `https://api.weather.gov/stations/{station_id}/observations/latest`
//!
//! Auth: none, but a `User-Agent` is required by NWS policy.
//!
//! We fetch a fixed list of 5 stations across the US. Any station
//! failing aborts the source per fail-loud MVP policy.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

use super::canonical as c;
use super::{CanonicalSample, EntropySource, RawSample};

pub const SOURCE: &str = "nws";

/// Fixed-order list of stations. Order is part of the spec — changing
/// it is a breaking change requiring a domain-tag bump.
pub const STATIONS: &[&str] = &["KDEN", "KJFK", "KLAX", "KORD", "KSEA"];

const USER_AGENT: &str = "random-dungeon/0.1 (https://github.com/random-dungeon)";

pub struct NwsSource {
    client: Client,
}

impl NwsSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn endpoint_for(station: &str) -> String {
        format!(
            "https://api.weather.gov/stations/{}/observations/latest",
            station
        )
    }
}

#[async_trait::async_trait]
impl EntropySource for NwsSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn fetch(&self) -> Result<RawSample> {
        // Fetch all stations in parallel; concatenate JSON objects into
        // a NDJSON-style payload (one observation per line, station_id
        // as the key) so the raw archive is deterministic.
        use futures::future::try_join_all;

        let futures = STATIONS.iter().map(|station| async move {
            let url = Self::endpoint_for(station);
            let resp = self
                .client
                .get(&url)
                .header("User-Agent", USER_AGENT)
                .header("Accept", "application/geo+json")
                .send()
                .await
                .with_context(|| format!("nws: request failed for {}", station))?;
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "nws: station {} returned {}",
                    station,
                    resp.status()
                ));
            }
            let body = resp
                .bytes()
                .await
                .with_context(|| format!("nws: read body for {}", station))?
                .to_vec();
            Ok::<(String, Vec<u8>), anyhow::Error>((station.to_string(), body))
        });

        let results: Vec<(String, Vec<u8>)> = try_join_all(futures).await?;

        // Build NDJSON: each line is {"station": "KJFK", "observation": <raw>}.
        // Sorted by station so the raw payload is itself deterministic.
        let mut sorted = results;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut payload: Vec<u8> = Vec::new();
        for (station, body) in &sorted {
            // Parse the raw observation just to confirm it's valid JSON,
            // but archive the original bytes (with whitespace normalized
            // via re-serialization to keep the archive small and stable).
            let v: serde_json::Value =
                serde_json::from_slice(body).context("nws: invalid JSON in observation")?;
            let line = serde_json::json!({ "station": station, "observation": v });
            payload.extend_from_slice(serde_json::to_string(&line)?.as_bytes());
            payload.push(b'\n');
        }

        Ok(RawSample {
            source: SOURCE,
            fetched_at_ms: Utc::now().timestamp_millis(),
            endpoint: "https://api.weather.gov/stations/{station}/observations/latest"
                .to_string(),
            payload,
        })
    }

    fn canonicalize(&self, raw: &RawSample) -> Result<CanonicalSample> {
        // Parse NDJSON-style payload into a station -> observation map.
        let mut by_station: std::collections::HashMap<String, NwsObservation> =
            std::collections::HashMap::new();

        for line in raw.payload.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }
            let entry: NwsArchiveLine =
                serde_json::from_slice(line).context("nws: parse archive line")?;
            by_station.insert(entry.station, entry.observation);
        }

        let mut buf = c::open(SOURCE);
        let count: u32 = STATIONS
            .len()
            .try_into()
            .map_err(|_| anyhow!("nws: too many stations"))?;
        c::write_u32_be(&mut buf, count);

        // Iterate in spec order, NOT sorted — stations are listed
        // explicitly in the spec.
        for station_id in STATIONS {
            let obs = by_station
                .get(*station_id)
                .ok_or_else(|| anyhow!("nws: missing observation for {}", station_id))?;
            let props = &obs.properties;

            c::write_lp_str(&mut buf, station_id)?;

            // Timestamp → ms since epoch.
            let observed_ms = c::parse_iso8601_ms(&props.timestamp)?;
            c::write_i64_be(&mut buf, observed_ms);

            // Temperature: °C × 1000.
            c::write_i32_be(
                &mut buf,
                c::float_to_fixed_i32(props.temperature.value, 1000.0),
            );

            // Pressure: pascals (already integer-ish, but reported as
            // float in the NWS API). × 1, no scaling.
            c::write_i32_be(
                &mut buf,
                c::float_to_fixed_i32(props.barometric_pressure.value, 1.0),
            );

            // Humidity: % × 10 → per-mille.
            c::write_i16_be(
                &mut buf,
                c::float_to_fixed_i16(props.relative_humidity.value, 10.0),
            );

            // Wind speed: m/s × 1000.
            c::write_i32_be(
                &mut buf,
                c::float_to_fixed_i32(props.wind_speed.value, 1000.0),
            );

            // Wind direction: degrees × 100 (centi-degrees).
            c::write_i32_be(
                &mut buf,
                c::float_to_fixed_i32(props.wind_direction.value, 100.0),
            );
        }

        Ok(CanonicalSample {
            source: SOURCE,
            bytes: buf,
        })
    }
}

#[derive(Debug, Deserialize)]
struct NwsArchiveLine {
    station: String,
    observation: NwsObservation,
}

#[derive(Debug, Deserialize)]
struct NwsObservation {
    properties: NwsProps,
}

#[derive(Debug, Deserialize)]
struct NwsProps {
    /// ISO-8601 / RFC-3339 timestamp.
    timestamp: String,
    temperature: NwsValue,
    #[serde(rename = "barometricPressure")]
    barometric_pressure: NwsValue,
    #[serde(rename = "relativeHumidity")]
    relative_humidity: NwsValue,
    #[serde(rename = "windSpeed")]
    wind_speed: NwsValue,
    #[serde(rename = "windDirection")]
    wind_direction: NwsValue,
}

#[derive(Debug, Deserialize)]
struct NwsValue {
    /// Reported value or null if the instrument is offline.
    value: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/nws/observations.ndjson");

    #[test]
    fn canonicalize_is_deterministic() {
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: "test".to_string(),
            payload: FIXTURE.as_bytes().to_vec(),
        };
        let src = NwsSource::new(Client::new());
        let a = src.canonicalize(&raw).unwrap();
        let b = src.canonicalize(&raw).unwrap();
        assert_eq!(a.bytes, b.bytes);
    }

    #[test]
    fn missing_station_fails() {
        // Drop one station from the NDJSON — should fail loud.
        let bad: String = FIXTURE
            .lines()
            .filter(|l| !l.contains("KJFK"))
            .collect::<Vec<_>>()
            .join("\n");
        let raw = RawSample {
            source: SOURCE,
            fetched_at_ms: 0,
            endpoint: "test".to_string(),
            payload: bad.into_bytes(),
        };
        let src = NwsSource::new(Client::new());
        assert!(src.canonicalize(&raw).is_err());
    }
}

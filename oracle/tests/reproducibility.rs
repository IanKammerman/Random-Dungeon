//! End-to-end reproducibility test.
//!
//! Loads recorded API responses from `tests/fixtures/`, runs them through
//! the full canonicalize → manifest → seed pipeline, and checks the
//! manifest hash and seed against pinned values.
//!
//! If this test fails, either:
//!   1. Someone changed canonicalization without bumping `DOMAIN_TAG` in
//!      `entropy/mod.rs` (this is a bug — fix the canonicalization), OR
//!   2. The change is intentional and the spec version was bumped (this
//!      is a breaking change — update `EXPECTED_*` below and document
//!      the migration in `docs/entropy.md`).

use oracle::entropy::{
    btc::BtcSource, drand::DrandSource, manifest, nws::NwsSource, seed, usgs::UsgsSource,
    EntropySource, RawSample,
};

const USGS_FIXTURE: &str = include_str!("fixtures/usgs/all_hour.json");
const NWS_FIXTURE: &str = include_str!("fixtures/nws/observations.ndjson");
const BTC_FIXTURE: &str = include_str!("fixtures/btc/latestblock.json");
const DRAND_FIXTURE: &str = include_str!("fixtures/drand/latest.json");

const FIXED_FETCHED_AT_MS: i64 = 1_746_712_900_000;
const EPOCH: u64 = 42;

/// Pinned expected values produced by the v1 spec on the checked-in
/// fixtures. Any change to canonicalization, manifest layout, or seed
/// derivation will change these. Update only when intentionally bumping
/// `DOMAIN_TAG` to a new version.
const EXPECTED_MANIFEST_HASH_HEX: &str =
    "fd37390f1a8f16f6be48fa69ed9ee1a012a8a4cdf9c685448c3696e3ad1f3f27";
const EXPECTED_SEED_HEX: &str =
    "a1b3832cd509c46e70f8c8f23ea13ee8b68b555b1ed23b5c34801023b167bf27";

fn raw(source: &'static str, payload: &str) -> RawSample {
    RawSample {
        source,
        fetched_at_ms: FIXED_FETCHED_AT_MS,
        endpoint: format!("fixture://{}", source),
        payload: payload.as_bytes().to_vec(),
    }
}

#[test]
fn pipeline_reproducibility() {
    let client = reqwest::Client::new();
    let usgs = UsgsSource::new(client.clone());
    let nws = NwsSource::new(client.clone());
    let btc = BtcSource::new(client.clone());
    let drand = DrandSource::new(client);

    let c_usgs = usgs.canonicalize(&raw("usgs", USGS_FIXTURE)).unwrap();
    let c_nws = nws.canonicalize(&raw("nws", NWS_FIXTURE)).unwrap();
    let c_btc = btc.canonicalize(&raw("btc", BTC_FIXTURE)).unwrap();
    let c_drand = drand.canonicalize(&raw("drand", DRAND_FIXTURE)).unwrap();

    let m = manifest::build(
        EPOCH,
        FIXED_FETCHED_AT_MS,
        &[c_usgs, c_nws, c_btc, c_drand],
    )
    .unwrap();
    let manifest_hash = manifest::hash(&m).unwrap();
    let s_t = seed::derive_seed(&manifest_hash);

    assert_eq!(
        hex::encode(manifest_hash),
        EXPECTED_MANIFEST_HASH_HEX,
        "manifest hash drift — see test docstring"
    );
    assert_eq!(
        hex::encode(s_t),
        EXPECTED_SEED_HEX,
        "seed drift — see test docstring"
    );
}

#[test]
fn pipeline_is_deterministic_across_runs() {
    let client = reqwest::Client::new();
    let usgs = UsgsSource::new(client.clone());
    let nws = NwsSource::new(client.clone());
    let btc = BtcSource::new(client.clone());
    let drand = DrandSource::new(client);

    let run = || {
        let c_usgs = usgs.canonicalize(&raw("usgs", USGS_FIXTURE)).unwrap();
        let c_nws = nws.canonicalize(&raw("nws", NWS_FIXTURE)).unwrap();
        let c_btc = btc.canonicalize(&raw("btc", BTC_FIXTURE)).unwrap();
        let c_drand = drand.canonicalize(&raw("drand", DRAND_FIXTURE)).unwrap();
        let m = manifest::build(
            EPOCH,
            FIXED_FETCHED_AT_MS,
            &[c_usgs, c_nws, c_btc, c_drand],
        )
        .unwrap();
        let h = manifest::hash(&m).unwrap();
        (h, seed::derive_seed(&h))
    };
    assert_eq!(run(), run());
}

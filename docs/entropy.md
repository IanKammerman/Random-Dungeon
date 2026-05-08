# Entropy Sources & Seed Derivation

This document is the **auditability contract** for Random Dungeon. Anyone
holding the on-chain record for an epoch plus the archived raw API responses
must be able to re-derive the seed `s_t` byte-for-byte and confirm it matches
what the oracle submitted on-chain.

If a future code change breaks reproducibility against this spec, that is a
bug in the code, not the spec. The spec is versioned (see "Domain
Separation" below); breaking changes require a new version tag.

## Goals

The seed `s_t` for epoch `t` must be:

1. **Unpredictable before epoch `t` opens** — its inputs come from physical
   or cryptographic processes outside any single party's control.
2. **Reproducible after the fact** — given the archived raw inputs and this
   spec, anyone re-derives the same `s_t`.
3. **Independent across sources** — manipulating one source does not
   meaningfully reduce the entropy of the seed.
4. **Unbiased** — no source's contribution can be silently re-rolled by the
   oracle to bias the output. (Achieved by committing to the manifest hash
   on-chain before the seed is consumed by the VRF.)

## Sources

The MVP uses four sources, fetched fresh each epoch.

### 1. USGS earthquake feed (`usgs`)

- **Endpoint:** `https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson`
- **Auth:** none.
- **What we extract:** the full set of earthquake events in the past hour.
- **Why it is good entropy:** the exact set of events, their magnitudes to
  two decimals, and their coordinates to milli-degree resolution come from
  global seismic activity and instrument noise. No single actor can
  produce, suppress, or predict micro-quakes.
- **Failure mode:** API down, malformed GeoJSON, or zero events in the
  past hour. MVP policy is **fail loud** — the oracle aborts the epoch
  with a structured error rather than continuing with degraded entropy.

### 2. NOAA / National Weather Service observations (`nws`)

- **Endpoint:** `https://api.weather.gov/stations/{station_id}/observations/latest`
- **Auth:** none. A `User-Agent` header identifying the oracle (e.g.
  `random-dungeon/0.1 (contact@example.com)`) is required by NWS policy.
- **Stations** (fixed list, sorted ascending by station id):
    - `KJFK` (New York, JFK)
    - `KLAX` (Los Angeles, LAX)
    - `KORD` (Chicago, O'Hare)
    - `KDEN` (Denver)
    - `KSEA` (Seattle)
- **What we extract per station:** observation timestamp, temperature,
  barometric pressure, relative humidity, wind speed, and wind direction,
  at full reported precision.
- **Why it is good entropy:** weather is chaotic; the bottom digits of
  reported instrument readings are at the noise floor and are not
  predictable by any external party. Five stations across the US
  decorrelate regional weather effects.
- **Failure mode:** any single station returning stale or missing data
  is a fetch failure for the whole `nws` source. Fail loud in MVP.

### 3. Bitcoin block hash (`btc`)

- **Endpoint (primary):** `https://blockchain.info/latestblock`
- **Endpoint (fallback):** `https://mempool.space/api/blocks/tip/hash`
- **Auth:** none.
- **What we extract:** the most recent Bitcoin block whose timestamp is
  at least `BTC_MIN_AGE_SECS = 600` (one block) before the start of the
  current epoch — i.e. one confirmation deep. We extract `(height, hash,
  time)`.
- **Why it is good entropy:** a Bitcoin block hash is the output of
  global proof-of-work and contains 256 bits of cryptographic entropy.
  Manipulating it costs more than any conceivable attack on the beacon.
- **Why one confirmation:** zero confirmations risks short reorgs
  changing the tip. One confirmation eliminates almost all reorg risk
  while keeping the source fresh per epoch.
- **Failure mode:** primary down → try fallback. Both down → fail loud.

### 4. drand public randomness beacon (`drand`)

- **Endpoint:** `https://api.drand.sh/public/latest`
  (default chain: League of Entropy mainnet)
- **Auth:** none.
- **What we extract:** the current `(round, randomness, signature)`.
- **Why it is good entropy:** drand is a threshold-BLS distributed
  randomness beacon run by ~18 organizations across four continents.
  Its output is unpredictable, unbiasable, and cryptographically
  verifiable against a public group key.
- **Note on independence:** Cloudflare's drand contribution incorporates
  LavaRand entropy. We treat drand as a single source rather than
  double-counting LavaRand.
- **Failure mode:** API down or signature verification fails → fail loud.

### Sources we considered and rejected

- **Sports scores:** manipulable, sparse during off-hours, low min-entropy
  in the relevant digits.
- **Stock / crypto market prices:** manipulable in last-digit precision by
  large traders.
- **NIST randomness beacon:** centralized; reintroduces the trust model
  we are trying to avoid.
- **Direct LavaRand / Cloudflare Workers `crypto.getRandomValues`:** no
  public verifiability — auditors cannot fetch yesterday's bytes. Already
  carried into drand under the threshold abstraction.

## Canonicalization

Each source emits a `RawSample` (the unmodified API response bytes plus
fetch metadata) and a `CanonicalSample` (a fixed-layout byte string
derived from the raw response). **We never hash JSON directly** — JSON
whitespace, key ordering, and floating-point formatting break
reproducibility. Canonicalization is the only path from raw bytes to
seed input.

All multi-byte integers are **big-endian**. All floats are converted to
fixed-point integers at the precision shown. Missing-but-allowed fields
use `i32::MIN` / `i64::MIN` / `u32::MAX` sentinels as noted.

### Domain separation

A single ASCII tag prefixes every canonical record:

```
DOMAIN_TAG = b"random-dungeon/entropy/v1"   // 25 bytes
```

The version suffix lets us evolve the spec without silently breaking
old audits — bumping to `v2` produces a different seed for the same
inputs.

### `usgs` canonical layout

```
usgs_canonical := DOMAIN_TAG || "usgs" || count_be_u32 || event[0] || event[1] || ...

event := id_len_be_u16
      || id_utf8_bytes                   // variable length
      || time_ms_be_i64                  // event time, ms since epoch
      || updated_ms_be_i64               // last-update time
      || mag_microunits_be_i32           // magnitude × 1_000_000, sentinel i32::MIN if null
      || lat_microdeg_be_i32             // latitude × 1_000_000
      || lon_microdeg_be_i32             // longitude × 1_000_000
      || depth_mm_be_i32                 // depth in millimeters
```

Events are sorted ascending by `id_utf8_bytes` (lexicographic on raw
bytes). The `count` is a `u32` count of events included.

### `nws` canonical layout

```
nws_canonical := DOMAIN_TAG || "nws" || count_be_u32 || station_record[0] || ...

station_record := station_id_len_be_u16
               || station_id_utf8_bytes
               || observed_ms_be_i64           // ISO-8601 timestamp → ms since epoch
               || temp_milliC_be_i32           // °C × 1000, sentinel i32::MIN if null
               || pressure_pa_be_i32           // pascals, integer, sentinel i32::MIN
               || humidity_permille_be_i16     // % × 10 (0..1000), sentinel i16::MIN
               || wind_speed_mms_be_i32        // m/s × 1000, sentinel i32::MIN
               || wind_dir_centideg_be_i32     // degrees × 100 (0..36000), sentinel i32::MIN
```

Stations are included in the order listed in the spec above (fixed,
not sorted), so a station outage produces a missing record (not a
silent drop). All five stations must succeed; otherwise the source
fails.

### `btc` canonical layout

```
btc_canonical := DOMAIN_TAG || "btc" || height_be_u32 || hash_bytes_32 || time_be_i64
```

`hash_bytes_32` is the block hash in raw 32-byte big-endian form (i.e.
the standard hex display, decoded). Note that Bitcoin RPC sometimes
returns hashes in display order (reverse of internal byte order); the
decoded value here is **display order**, matching `blockchain.info`'s
output.

### `drand` canonical layout

```
drand_canonical := DOMAIN_TAG || "drand" || round_be_u64 || randomness_bytes_32 || signature_len_be_u16 || signature_bytes
```

Signature length is variable across drand chains (BLS12-381 G1 = 48 bytes
on mainnet's unchained beacon); we record the length explicitly to keep
the spec chain-agnostic.

## Manifest

After all four canonical samples are built, the oracle constructs a
manifest binding the epoch number, the per-source canonical hashes,
and a fetch timestamp. The manifest hash is what goes on-chain.

```
canonical_hash_i := SHA256(canonical_i)

manifest := DOMAIN_TAG
         || "manifest"
         || epoch_be_u64
         || fetched_at_ms_be_i64
         || source_count_be_u32         // = 4 in MVP
         || record[0] || record[1] || record[2] || record[3]

record := source_name_len_be_u16
       || source_name_utf8              // "btc" | "drand" | "nws" | "usgs"
       || canonical_hash_32

manifest_hash := SHA256(manifest)
```

Records are sorted **ascending by source name** so manifest layout is
deterministic regardless of fetch completion order.

## Seed derivation

The seed is bound to the manifest, not the raw canonical bytes
directly. This means an auditor only needs the manifest plus
per-source canonical bytes — not the full raw responses — to verify
`s_t`:

```
s_t := SHA256(DOMAIN_TAG || "seed" || manifest_hash)
```

This indirection is intentional. The on-chain record commits to
`manifest_hash`; reproducibility of `s_t` from `manifest_hash` is one
SHA-256 call. Reproducibility of `manifest_hash` from raw archived
responses is the spec above.

## On-chain record (per epoch)

The off-chain oracle submits, alongside the VRF output:

- `epoch_id: u64`
- `entropy_manifest_hash: [u8; 32]`
- `entropy_seed: [u8; 32]`             // = s_t, input to VRF
- (existing fields: vrf_output, vrf_proof, etc.)

Coordinate with the on-chain program owner: `EpochState` needs the
`entropy_manifest_hash` and `entropy_seed` fields added.

## Archival

For the MVP:

- Raw responses are written to
  `oracle/archive/<epoch>/<source>.json` with an accompanying
  `oracle/archive/<epoch>/manifest.json` containing the structured
  manifest.
- A subset of recorded responses is committed under
  `oracle/tests/fixtures/<source>/` and used to pin the
  canonicalization in unit tests. Any change in canonicalization that
  breaks a fixture is a breaking spec change.
- Stretch goal: pin manifests + raw bundles to IPFS or Arweave; record
  the CID/tx-id alongside `entropy_manifest_hash` on-chain.

## Reproducibility test

A passing `cargo test --test reproducibility` proves: given the
fixtures in `oracle/tests/fixtures/`, the canonicalization and seed
derivation produce a known-good `manifest_hash` and `s_t`. This test
is what prevents the spec and code from silently drifting.

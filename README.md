#Random Dungeon
# Verifiable Randomness Beacon on Solana

A publicly verifiable randomness beacon on Solana.

**Team:** Tony Fields, Ian Kammerman, Cathy Zhang
**Course:** COMS 4995 — Sciences of Blockchain

## Overview

TODO: one-paragraph description of what the beacon does and who it's for.

## Design

TODO: high-level description of how the system works end-to-end.

### On-Chain (Solana Program)

The deployed program is the verifier and record of truth. It does not fetch data or hold secrets — it only accepts submissions and verifies them.

**State per epoch**
- Epoch number, phase, and deadline slots (commit / reveal / finalize)
- Oracle seed commitment, then revealed seed
- Participant commitments and verified reveals
- Finalized: `v_t`, VRF proof bytes, Merkle root of contributions
- Registered oracle public key (for VRF verification)

**Instructions**
- `initialize_epoch` — start new epoch, set deadlines
- `oracle_commit` / `oracle_reveal` — oracle's seed commitment and reveal
- `participant_commit` / `participant_reveal` — optional n-party contributions
- `finalize_epoch` — submit `v_t` and VRF proof; program verifies and stores

**Enforced logic**
- Phase transitions by slot number
- Commitment verification (reveal must hash to commitment)
- VRF proof verification via Solana's ed25519 precompile
- Seed aggregation over valid reveals; non-revealers dropped

### Off-chain Oracle

The off-chain oracle service submits entropy seeds to the Solana program via a Helius RPC endpoint, which provides reliable transaction submission and devnet/mainnet access without running a full validator. The RPC URL is configured via the `SOLANA_RPC_URL` environment variable so providers can be swapped without code changes.

A service we run that translates real-world data into on-chain submissions. Holds the ed25519 VRF keypair.

**Per-epoch flow**
1. Detect new epoch via RPC
2. Submit `oracle_commit` with `H(salt)` before commit deadline
3. Poll entropy APIs (NOAA, Open-Meteo, USGS, sports)
4. Submit `oracle_reveal` with seed derived from API responses and salt
5. Wait for reveal window to close
6. Read aggregated seed from program state
7. Compute VRF: `(v_t, proof) = VRF_sign(sk, aggregated_seed)`
8. Submit `finalize_epoch` with `v_t` and proof

### Entropy Sources

NASA API: Sun spots cant be controlled by any person, and its through NASA which is a reliable source.
Weather API (NOAA/weather.gov API): Hard to manipulate, random and unpredictable.


### VRF

TODO: which VRF, keying model.

### Contribution Protocol

TODO: commit-reveal spec, non-revealer fallback.

### Epoch Timing

Each epoch runs for approximately 5–10 minutes in the MVP, with an optimization target of 1–2 minutes. An epoch consists of three phases:

- **Commit phase** — oracle and participants submit hash commitments. Duration: ~40% of epoch.
- **Reveal phase** — oracle and participants reveal preimages; program verifies against commitments. Duration: ~40% of epoch.
- **Finalize phase** — oracle submits VRF output and proof; program verifies and records. Duration: ~20% of epoch.

Phase boundaries are enforced by Solana slot number, not wall-clock time. Participants who miss the reveal deadline are silently dropped and the epoch proceeds with remaining valid reveals.

### On-Chain Storage

Per-epoch storage is kept minimal. For each finalized epoch, the program stores:

- Epoch number
- Final beacon output `v_t`
- VRF proof bytes
- Merkle root of participant contributions
- Oracle public key used for this epoch

Transient state during an epoch (active commitments, pending reveals, phase deadlines) is stored in a separate working account and can be cleared after finalization. Raw API inputs are not stored on-chain; in the stretch target, their content hashes are stored on-chain with the raw data archived to IPFS or Arweave.

## Milestones

### MVP

TODO

### Realistic Target

TODO

### Stretch Target

TODO

## Security Considerations

TODO

## Why Solana

TODO

## Related Work

TODO

## Getting Started

TODO: build and run instructions.

## License

Apache 2.0
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

Elliptic Curve Verifiable Random Function + SNARK
We compute the VRF normally, then use a SNARK to prove that computation was done correctly, turning the VRF into a composable, private, and programmable building block.

### ECVRF SNARK MVP

This repo now includes an MVP BN254 Groth16 pipeline under `crates/`:

- `vrf-core` computes a deterministic field-based VRF-like value.
- `vrf-circuit` proves the matching field arithmetic in R1CS.
- `setup` generates local Arkworks proving/verifying keys or imports ceremony artifacts.
- `prover` creates a Groth16 proof and fixed-order public inputs.
- `verifier-client` verifies proofs locally with Arkworks.
- `ecvrf-solana-program` wires an Anchor instruction to `groth16-solana`.

The MVP intentionally does not implement full RFC 9381 ECVRF inside the circuit. It proves:

```text
h = Poseidon(alpha_hash)
gamma = sk * h
beta = Poseidon(gamma)
```

Public input order is fixed as:

```text
[alpha_hash, beta]
```

The current circuit is still scalar-field-only. A later ECVRF upgrade should replace `h = Poseidon(alpha_hash)` with hash-to-curve and prove `Gamma = sk * H`.

For handoff/testing/integration details, see [docs/snark-vrf-integration.md](docs/snark-vrf-integration.md).

### Contribution Protocol

The beacon's baseline entropy comes from the oracle. Any Solana wallet may additionally contribute entropy to an epoch via a commit-reveal scheme. Contributions are optional, the beacon finalizes with or without them, but allow consumers to reduce their trust in the oracle: a contributor who keeps their `r_i` secret until reveal has cryptographic assurance that the epoch's output incorporates their own randomness, regardless of the oracle's behavior.

Participants do not pre-register. To contribute to epoch `t`, a wallet submits a `participant_commit` transaction during epoch `t`'s commit phase and a `participant_reveal` transaction during the reveal phase.
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

The local random setup mode is for development only.

Production Groth16 deployments require a ceremony. The security goal is that the toxic waste used to generate the proving and verifying keys is unknown after setup. If the toxic waste is retained, an attacker may forge proofs.

The Powers of Tau phase can be reused across circuits up to a supported circuit size. The Groth16 phase 2 setup is circuit-specific and must be rerun whenever the circuit changes.

Groth16 is attractive here because proofs are very small and verification is efficient, but its setup is circuit-specific.

## Why Solana

TODO

## Related Work

TODO

## Getting Started

Generate local development keys:

```bash
cargo run -p setup -- local-random
```

Prove the MVP VRF computation:

```bash
cargo run -p prover -- --sk 12345 --alpha "user supplied randomness input"
```

This writes both Arkworks-local verification files and Solana-ready byte files:

```text
artifacts/proof.bin                  # compressed Arkworks proof for verifier-client
artifacts/public_inputs.json          # hex public inputs for verifier-client
artifacts/proof_solana.bin            # 256 bytes: -A || B || C for groth16-solana
artifacts/public_inputs_solana.json   # [[u8; 32]; 2] in [alpha_hash, beta] order
artifacts/public_inputs_solana.bin    # 64 bytes: alpha_hash || beta
```

Verify locally:

```bash
cargo run -p verifier-client -- \
  --proof artifacts/proof.bin \
  --public-inputs artifacts/public_inputs.json \
  --vk artifacts/verifying_key.bin
```

Import ceremony output after running the `snarkjs` flow in [ceremony/README.md](ceremony/README.md):

```bash
cargo run -p setup -- import-ceremony \
  --zkey ceremony/zkey/vrf_0001.zkey \
  --vk-json artifacts/verifying_key.json
```

Run core tests:

```bash
cargo test -p vrf-core
cargo test -p vrf-circuit
cargo test -p setup
cargo test -p prover
cargo test -p verifier-client
```

## License

Apache 2.0

# Random Dungeon

## Verifiable Randomness Beacon on Solana

Random Dungeon is a Solana randomness beacon that combines a commit-reveal oracle, external entropy samples, and a Groth16-verified VRF-style computation. The on-chain program records epoch state, enforces timing and oracle authority, verifies the proof, and stores the finalized beacon output for downstream consumers.

**Team:** Tony Fields, Ian Kammerman, Cathy Zhang
**Course:** COMS 4995 - Sciences of Blockchain

## Overview

Each epoch starts with an oracle commitment, moves through a reveal window where the oracle binds its salt to an entropy manifest, and closes when the oracle submits a VRF output plus a Groth16 proof. The program is intentionally small: it does not fetch web data or hold secrets. It validates commitments, checks the registered oracle signer, verifies the SNARK proof, and stores the output.

## Design

The protocol splits responsibility between the on-chain verifier and an off-chain oracle:

1. `initialize_epoch` creates an epoch account, records the oracle pubkey, and sets slot deadlines.
2. `oracle_commit` stores `SHA256(salt)` during the commit phase.
3. The oracle gathers entropy from external sources and builds a canonical manifest.
4. `oracle_reveal` reveals `(salt, manifest_hash)`, verifies the salt commitment, and derives the epoch seed as `SHA256("random-dungeon/oracle-seed/v1" || salt || manifest_hash)`.
5. `finalize_epoch` verifies a Groth16 proof for the VRF-style computation and stores `v_t`.

### On-Chain Program

The Anchor program in `programs/randomness-beacon` owns the epoch state and enforces:

- Slot-gated commit, reveal, and finalize windows.
- Oracle-only access for `oracle_commit`, `oracle_reveal`, and `finalize_epoch`.
- Oracle salt commitment verification.
- Storage of the entropy manifest hash, derived entropy seed, VRF output, and finalization status.
- Groth16 verification through `groth16-solana`.

### Off-Chain Oracle

The Rust oracle in `oracle/` watches a configured epoch PDA through `SOLANA_RPC_URL`. In run mode it reads the epoch state, submits the commit, waits for reveal phase, builds an entropy bundle, archives the raw responses under `oracle/archive/<epoch>/`, and submits the reveal.

The finalize path depends on the prover artifacts and VRF secret. The current prover path is documented in `docs/snark-vrf-integration.md`; the teammate-facing local demo review guide is in `docs/local-demo-review.md`, with a manual checklist in `docs/demo-walkthrough.md`.

### Entropy Sources

Cathy's entropy module canonicalizes several external sources into a reproducible manifest. The seed derivation is intentionally two-stage: the entropy module derives a manifest-bound seed for audit, and the protocol reveal mixes `manifest_hash` with the precommitted oracle salt before storing the on-chain epoch seed.

## VRF

The MVP uses a BN254 scalar-field VRF-like computation wrapped in Groth16:

```text
alpha_hash = SHA256(alpha) reduced into Fr
h = Poseidon(alpha_hash)
gamma = sk * h
beta = Poseidon(gamma)
```

Public inputs are fixed as:

```text
[alpha_hash, beta]
```

The current circuit is not a full RFC 9381 ECVRF. A future upgrade should replace the scalar-field shortcut with hash-to-curve and prove `Gamma = sk * H(alpha)`.

## Security Considerations

The oracle key is registered per epoch during `initialize_epoch`; commit, reveal, and finalize reject any other signer. This prevents arbitrary wallets from replacing the oracle's commitment, entropy reveal, or final VRF output.

The local random Groth16 setup is for development only. Production deployments require a real trusted setup ceremony where the toxic waste is destroyed. If the toxic waste survives, an attacker could forge proofs. The Powers of Tau phase can be reused across circuits up to its supported size, but Groth16 phase 2 is circuit-specific and must be rerun whenever the circuit changes.

The entropy manifest is not stored in full on-chain. The program stores only `manifest_hash` and the derived seed; raw API responses are archived locally for audit. A production deployment should pin manifests and raw source payloads to IPFS, Arweave, or another durable archive.

Participant commit/reveal is planned but optional for the MVP. Until it is wired, consumers trust that the registered oracle followed the entropy sampling procedure represented by the archived manifest.

## Why Solana

Solana gives this design cheap, low-latency verification and a natural account model for epoch state. The beacon can publish frequent outputs without making proof verification or state updates prohibitively expensive for consumers.

## Milestones

### MVP

- Commit/reveal oracle with oracle-only access control.
- Canonical entropy manifest and seed derivation.
- Local Groth16 setup, prover, verifier client, and on-chain verifier path.
- Local validator demo walkthrough.
- Fully scripted local demo from validator startup through finalization.

### Realistic Target

- Unignored end-to-end oracle integration test.
- Durable archive for entropy manifests and raw source payloads.
- Clear operational docs for devnet deployment.

### Stretch Target

- Participant commit/reveal contributions.
- Anchor 0.31+ upgrade to remove the `--no-idl` workaround.
- Full ECVRF circuit rather than the scalar-field MVP.
- Production-grade trusted setup ceremony.

## Getting Started

Install Rust, the Solana CLI, and Anchor 0.30.1:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
cargo install --git https://github.com/coral-xyz/anchor avm
avm install 0.30.1
avm use 0.30.1
```

Add the toolchains to your shell profile:

```bash
export PATH="$HOME/.cargo/bin:$HOME/.local/share/solana/install/active_release/bin:$PATH"
```

Generate a local wallet if needed:

```bash
solana-keygen new --outfile ~/.config/solana/id.json --no-bip39-passphrase
```

Copy `.env.example` to `.env` and set:

```text
SOLANA_RPC_URL=http://localhost:8899
ORACLE_KEYPAIR_PATH=~/.config/solana/id.json
PROGRAM_ID=9Trpfw7P4YzbaaRQYDS5fmnsAGie5JLQ1FjcgzgJfDq9
EPOCH_ID=1
ORACLE_VRF_SECRET=0x...
PROVER_BINARY_PATH=target/release/prover
PROVING_KEY_PATH=artifacts/proving_key.bin
```

Build with the current Anchor workaround:

```bash
anchor build --no-idl
anchor test --skip-build
```

Anchor 0.30.1 with newer Rust has a known IDL-generation issue through `proc-macro2`, so use `anchor build --no-idl` for now. If `anchor test --skip-build` ends with a trailing `Error: No such file or directory` after tests pass, that is known post-test wrapper noise.

Generate local proving artifacts:

```bash
cargo run -p setup -- local-random
```

Prove and verify the MVP VRF computation:

```bash
cargo run -p prover -- --sk 12345 --alpha "test randomness"
cargo run -p verifier-client -- \
  --proof artifacts/proof.bin \
  --public-inputs artifacts/public_inputs.json \
  --vk artifacts/verifying_key.bin
```

Run focused Rust tests:

```bash
cargo test -p vrf-core
cargo test -p vrf-circuit
cargo test -p setup
cargo test -p prover
cargo test -p verifier-client
cargo test -p oracle
```

For the full local end-to-end demo, use the teammate review guide:

```bash
unset ORACLE_VRF_SECRET
unset EPOCH_ID
./scripts/local-demo.sh
```

Expected success lines include `oracle_commit confirmed`, `oracle_reveal confirmed`, `finalize_epoch confirmed`, and `[OK] Epoch <id> completed successfully.` See `docs/local-demo-review.md` for what each step is doing and what to inspect.

## Related Work

- Chainlink VRF for production oracle-delivered randomness.
- drand for public threshold randomness.
- Solana `groth16-solana` examples for efficient BN254 proof verification.
- RFC 9381 ECVRF as the target shape for a fuller circuit.

## License

Course project code. Add a project license before publishing beyond the class context.

# SNARK VRF Integration Guide

This guide explains how to test and integrate the current SNARK-backed VRF MVP.

## What Exists

The current implementation proves this scalar-field computation over BN254:

```text
alpha_hash = SHA-256(alpha) reduced into Fr
h = Poseidon(alpha_hash)
gamma = sk * h
beta = Poseidon(gamma)
```

The Groth16 public inputs are always:

```text
[alpha_hash, beta]
```

This is not full RFC 9381 ECVRF yet. It is the working MVP path for proving a private scalar computation and verifying that proof on Solana.

## Crates

- `crates/vrf-core`: native VRF-like computation and public input serialization.
- `crates/vrf-circuit`: R1CS circuit for the same Poseidon computation.
- `crates/setup`: local random trusted setup and verifying-key export.
- `crates/prover`: proof generation and Solana byte export.
- `crates/verifier-client`: local Arkworks proof verification.
- `crates/solana-program`: Anchor program that verifies `groth16-solana` proof bytes.
- `crates/solana-local-validator-test`: standalone local-validator test harness.

The local-validator test harness is intentionally excluded from the root workspace because `solana-program-test` has a large Solana 2.2 dependency graph that conflicts with the existing oracle Solana 1.18 stack.

## One-Time Setup

Install Rust and make sure `cargo` is on your PATH:

```bash
source "$HOME/.cargo/env"
cargo --version
```

If you use a fresh terminal and `cargo` is missing, run the `source` command again or add it to your shell profile.

## Generate Dev Keys and Proof

From the repo root:

```bash
cargo run -p setup -- local-random
cargo run -p prover -- --sk 12345 --alpha "test randomness"
```

Expected generated files:

```text
artifacts/proving_key.bin              # local dev proving key, ignored by git
artifacts/verifying_key.bin            # local dev Arkworks verifying key, ignored by git
artifacts/verifying_key_solana.rs      # generated Solana VK constants, tracked
artifacts/proof.bin                    # Arkworks proof for verifier-client, ignored
artifacts/public_inputs.json           # Arkworks public inputs, ignored
artifacts/proof_solana.bin             # 256 bytes: -A || B || C, ignored
artifacts/public_inputs_solana.json    # [[u8; 32]; 2], ignored
artifacts/public_inputs_solana.bin     # 64 bytes: alpha_hash || beta, ignored
```

Important: `local-random` setup is for development only. It produces toxic waste and is not production safe.

## Local Verification

Verify the Arkworks proof locally:

```bash
cargo run -p verifier-client
```

Expected output:

```text
valid
```

Check the Solana proof file sizes:

```bash
wc -c artifacts/proof_solana.bin artifacts/public_inputs_solana.bin
```

Expected:

```text
256 artifacts/proof_solana.bin
 64 artifacts/public_inputs_solana.bin
```

## Test Commands

Run the core Rust tests:

```bash
cargo test -p vrf-core
cargo test -p vrf-circuit
cargo test -p setup
cargo test -p prover
cargo test -p verifier-client
cargo test -p ecvrf-solana-program
```

Run the local-validator test:

```bash
cargo test --manifest-path crates/solana-local-validator-test/Cargo.toml --test local_validator
```

Expected result:

```text
test local_validator_accepts_real_vrf_proof ... ok
```

Do not paste that output line into bash. `test` is a shell builtin, so bash will try to run it as a command.

## What The Local-Validator Test Proves

The local-validator test:

1. Starts `solana-program-test`.
2. Registers `crates/solana-program` as a native test program.
3. Submits a real 256-byte Groth16 proof exported by `crates/prover`.
4. Submits public inputs in `[alpha_hash, beta]` order.
5. Calls the Anchor instruction `verify_vrf_proof`.
6. Checks that the on-chain `VrfProofRecord` stores:
   - `accepted = true`
   - the caller authority
   - the accepted `beta`
   - the public inputs

The test fixture is tied to the currently tracked `artifacts/verifying_key_solana.rs`. If the circuit or verifying key changes, regenerate the proof fixture and update the test constants.

## On-Chain Instruction Interface

The verifier program exposes:

```rust
pub fn verify_vrf_proof(
    ctx: Context<VerifyVrfProof>,
    proof: Vec<u8>,
    public_inputs: Vec<[u8; 32]>,
) -> Result<()>
```

Expected proof format:

```text
proof[0..64]    = -A in Solana BN254 G1 byte format
proof[64..192]  = B in Solana BN254 G2 byte format
proof[192..256] = C in Solana BN254 G1 byte format
```

Expected public inputs:

```text
public_inputs[0] = alpha_hash
public_inputs[1] = beta
```

The program uses hardcoded constants from:

```text
artifacts/verifying_key_solana.rs
```

Regenerate that file after any circuit change:

```bash
cargo run -p setup -- local-random
```

Then regenerate proof fixtures:

```bash
cargo run -p prover -- --sk 12345 --alpha "test randomness"
```

## Oracle Integration Notes

The oracle currently still has a VRF stub in:

```text
oracle/src/vrf.rs
```

Recommended integration shape:

1. Treat the aggregated epoch seed as `alpha`.
2. Convert the oracle secret into a BN254 `Fr` secret scalar.
3. Use `vrf-core` to compute `alpha_hash` and `beta`.
4. Use the prover path to generate:
   - `proof_solana.bin` bytes
   - `public_inputs_solana` as `Vec<[u8; 32]>`
5. Submit those to the Solana verifier instruction.

For production code, do not shell out to `cargo run -p prover`. Move the prover logic into a library crate or shared function so the oracle can call it directly.

## Git Notes

Generated binary artifacts should not be committed:

```text
artifacts/proof.bin
artifacts/proof_solana.bin
artifacts/proving_key.bin
artifacts/public_inputs_solana.bin
artifacts/verifying_key.bin
```

The generated Solana verifying key source should be committed for now:

```text
artifacts/verifying_key_solana.rs
```

If git still shows tracked binary artifacts, untrack them while keeping local copies:

```bash
git rm --cached artifacts/proof.bin artifacts/proving_key.bin artifacts/verifying_key.bin
```

## Production Ceremony Path

The local setup is insecure. Production Groth16 requires ceremony output. See:

```text
ceremony/README.md
```

The high-level production path is:

1. Compile the equivalent Circom circuit.
2. Run Powers of Tau phase 1.
3. Run circuit-specific Groth16 phase 2.
4. Export `verification_key.json`.
5. Import/generate Solana verifying-key constants.


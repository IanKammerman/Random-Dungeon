# Local Demo Walkthrough

This is the manual validation path for a local validator demo. It is written as a checklist because the full commit -> reveal -> finalize flow depends on local validator state, generated proving artifacts, and the oracle keypair.

## Prerequisites

Install and select the expected toolchain:

```bash
rustc --version
solana --version
anchor --version
avm use 0.30.1
```

Use the current Anchor workaround:

```bash
anchor build --no-idl
anchor test --skip-build
```

The trailing `Error: No such file or directory` sometimes printed after `anchor test --skip-build` is cosmetic if the tests have already passed.

## Environment

Create a local wallet if one does not exist:

```bash
solana-keygen new --outfile ~/.config/solana/id.json --no-bip39-passphrase
```

Export the local environment:

```bash
export SOLANA_RPC_URL=http://localhost:8899
export ORACLE_KEYPAIR_PATH=~/.config/solana/id.json
export PROGRAM_ID=9Trpfw7P4YzbaaRQYDS5fmnsAGie5JLQ1FjcgzgJfDq9
export EPOCH_ID=1
export ORACLE_VRF_SECRET=0x...
export PROVER_BINARY_PATH=target/release/prover
export PROVING_KEY_PATH=artifacts/proving_key.bin
```

## Validator And Program

Start the validator in one terminal:

```bash
solana-test-validator --reset
```

Build and deploy in another terminal:

```bash
anchor build --no-idl
anchor deploy
```

Generate local proving artifacts:

```bash
cargo run -p setup -- local-random
```

## Epoch Setup

Initialize an epoch with the oracle wallet as the authority. The current repository exposes the instruction on-chain; if there is not yet a CLI wrapper, use an Anchor client/script or test harness to call:

```text
initialize_epoch(epoch_id, commit_deadline_slot, reveal_deadline_slot, finalize_deadline_slot)
```

Use deadline slots far enough apart to give the oracle process time to commit, gather entropy, reveal, prove, and finalize.

## Commit And Reveal

Run the oracle:

```bash
cargo run -p oracle
```

Expected behavior:

- The oracle exits early if the epoch PDA is missing.
- During commit phase it submits `oracle_commit` with `SHA256(salt)`.
- It waits for reveal phase.
- It gathers entropy, writes `oracle/archive/<epoch>/`, and submits `oracle_reveal(salt, manifest_hash)`.
- The program verifies the salt commitment and stores `entropy_manifest_hash`, `entropy_seed`, and `aggregated_seed`.

## Finalize

Generate a proof using the stored epoch seed as the alpha input. The current prover accepts string alpha input, so a finalize wrapper should pass the exact seed bytes or a stable hex representation and then submit:

```text
finalize_epoch(vrf_output, proof_solana.bin, public_inputs_solana.json)
```

Manual checks after finalization:

- `is_finalized == true`
- `phase == Closed`
- `vrf_output == public_inputs[1]`
- Unauthorized wallets cannot call commit, reveal, or finalize.
- A reveal with the wrong salt fails.

## Known Gaps

- The oracle binary currently automates commit and reveal, not finalize.
- The finalize wrapper still needs to bridge `ORACLE_VRF_SECRET`, prover artifacts, and on-chain transaction submission.
- Participant commit/reveal remains optional MVP work.
- A production demo should archive or pin the entropy manifest outside the local filesystem.

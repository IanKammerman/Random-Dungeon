# Local Demo Review Guide

This guide is for teammates reviewing the working local randomness beacon demo. It explains how to run the demo, what success looks like, and what each part of the script is doing.

The short version: `./scripts/local-demo.sh` starts or reuses a local Solana validator, builds and deploys the Anchor program, initializes one epoch, runs the oracle through commit/reveal/finalize, generates a Groth16 proof, verifies it on-chain, and writes logs for review.

## What This Demo Proves

- The Anchor program can be built and deployed to a local validator.
- An epoch can be initialized with slot deadlines and an oracle authority.
- The oracle can submit a commit before the commit deadline.
- The oracle can gather external entropy, archive the manifest, and reveal a salt plus manifest hash.
- The on-chain program verifies the salt commitment and derives the epoch seed.
- The oracle can generate a Groth16 proof for the VRF-style computation.
- The on-chain program verifies the proof and finalizes the epoch.

This is a local MVP demo. It does not prove production readiness, devnet deployment, participant commit/reveal, or a production trusted setup ceremony.

## Prerequisites

From the repository root, confirm these tools exist:

```bash
rustc --version
solana --version
anchor --version
avm use 0.30.1
```

If you do not already have a local Solana wallet:

```bash
solana-keygen new --outfile ~/.config/solana/id.json --no-bip39-passphrase
```

Anchor 0.30.1 currently needs the no-IDL build path in this repo:

```bash
anchor build --no-idl
```

## Fast Run

Use a fresh epoch id and the demo VRF secret:

```bash
unset ORACLE_VRF_SECRET
unset EPOCH_ID
./scripts/local-demo.sh
```

The run takes a few minutes because the oracle waits for local validator slots to move through commit, reveal, and finalize windows.

If you want a shorter review run, use smaller slot offsets:

```bash
unset ORACLE_VRF_SECRET
unset EPOCH_ID
COMMIT_OFFSET_SLOTS=60 REVEAL_OFFSET_SLOTS=140 FINALIZE_OFFSET_SLOTS=260 ./scripts/local-demo.sh
```

## Expected Success Output

A successful run includes these lines:

```text
Deploy success
initialize_epoch transaction confirmed
oracle_commit confirmed
oracle_reveal confirmed
finalize_epoch confirmed
[OK] Epoch <id> completed successfully.
```

The script also prints the full log path:

```text
[OK] Full log: target/local-demo-logs/local-demo-YYYYMMDD-HHMMSS.log
```

Use that log when comparing results across machines.

## What The Script Does

The script is [scripts/local-demo.sh](../scripts/local-demo.sh). It runs the full local demo path.

1. **Checks required commands**
   It verifies `cargo`, `solana`, `solana-keygen`, `solana-test-validator`, and `anchor` are available.

2. **Normalizes environment**
   It sets defaults for:
   - `SOLANA_RPC_URL=http://localhost:8899`
   - `ORACLE_KEYPAIR_PATH=~/.config/solana/id.json`
   - `PROGRAM_ID`
   - `EPOCH_ID`
   - `PROVER_BINARY_PATH=target/release/prover`
   - `PROVING_KEY_PATH=artifacts/proving_key.bin`
   - `RUST_LOG=oracle=info`

   If `ORACLE_VRF_SECRET` is unset or still a placeholder like `0x...`, the script uses a local demo scalar. If a custom value is set, it must be valid even-length hex and no more than 32 bytes.

3. **Starts or reuses a validator**
   If nothing is running at `SOLANA_RPC_URL`, it starts `solana-test-validator --reset` with logs under `target/local-demo-logs/`. If a validator is already running, it reuses it.

4. **Funds the oracle wallet**
   It airdrops local SOL to the oracle wallet so that wallet can deploy the program and pay transaction fees.

5. **Generates local Groth16 artifacts**
   It runs:

   ```bash
   cargo run -p setup -- local-random
   ```

   This creates local proving and verifying artifacts. This setup is for development only and is not production safe.

6. **Syncs Anchor program ids**
   It runs `anchor keys sync` before building so `Anchor.toml`, `declare_id!`, and `target/deploy/randomness_beacon-keypair.json` agree. This avoids the `0x1004` declared-program-id mismatch error.

   The script backs up and restores source files on exit by default, so this local id sync does not stay in the working tree.

7. **Builds and deploys**
   It builds the prover, oracle, and Anchor program, then deploys the program to the local validator.

8. **Initializes the epoch**
   It reads the current slot, computes future deadline slots, and calls:

   ```text
   initialize_epoch(epoch_id, commit_deadline_slot, reveal_deadline_slot, finalize_deadline_slot)
   ```

   The oracle wallet becomes the authorized oracle for that epoch.

9. **Runs commit/reveal/finalize**
   The oracle process then:
   - Creates a random salt and submits `oracle_commit(SHA256(salt))`.
   - Waits for the reveal phase.
   - Fetches entropy, builds a canonical manifest, and archives raw data under `oracle/archive/<epoch>/`.
   - Submits `oracle_reveal(salt, manifest_hash)`.
   - Waits for the finalize phase.
   - Uses the epoch seed as raw `--alpha-hex` input to the prover.
   - Submits `finalize_epoch(vrf_output, proof, public_inputs)`.

10. **Cleans up**
    If the script started the validator, it stops it unless `KEEP_VALIDATOR=1` is set. It also restores temporary source changes unless `RESTORE_ANCHOR_KEYS=0` is set.

## What To Inspect After A Run

Check the log file printed at the end. The most important line is:

```text
finalize_epoch confirmed
```

Check the archived entropy bundle:

```bash
ls oracle/archive/<epoch-id>/
```

Expected archive contents include raw source records and a manifest file. The manifest hash printed during reveal should match the manifest used to derive the on-chain epoch seed.

Check the working tree:

```bash
git status --short
```

The demo script should not leave `Anchor.toml` or `programs/randomness-beacon/src/lib.rs` changed from local program-id syncing. If generated artifacts were already dirty before the run, they may still show as modified.

## Useful Review Commands

Run these before approving or merging:

```bash
cargo test -p oracle
cargo test -p prover
bash -n scripts/local-demo.sh
git diff --check
```

Ignored integration tests that say they require `anchor build --no-idl` are expected unless you are intentionally running the validator-backed test suite.

## Troubleshooting

**`ORACLE_VRF_SECRET is not a valid BN254 scalar`**

Unset the placeholder:

```bash
unset ORACLE_VRF_SECRET
./scripts/local-demo.sh
```

The script now treats `0x...` and `...` as placeholders and uses the local demo secret.

**`custom program error: 0x1004` during `init-epoch`**

This means the deployed program id and compiled `declare_id!` do not match. Run the current script instead of manual deploy steps; it calls `anchor keys sync` before building.

**`Attempt to load a program that does not exist`**

The oracle is pointing at a different `PROGRAM_ID` than the one deployed to the local validator. The script parses the deployed program id and exports it for the local run.

**`Account already in use` or epoch initialization fails**

You probably reused an `EPOCH_ID` against a validator that already has that epoch PDA. Use a fresh epoch:

```bash
unset EPOCH_ID
./scripts/local-demo.sh
```

**Repeated `admin.rpc does not exist` lines in validator logs**

Those lines are local validator startup noise. If the demo reaches `Deploy success` and later confirms transactions, they are not the cause of failure.

## Known Limitations

- The Groth16 `local-random` setup is development-only.
- Participant commit/reveal is not wired yet.
- Entropy archives are local files; a production path should pin them to durable storage.
- The VRF circuit is an MVP scalar-field construction, not a full RFC 9381 ECVRF.
- Devnet/mainnet deployment still needs a separate operations runbook.

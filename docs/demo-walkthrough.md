# Local Demo Walkthrough

This is the manual validation path for a local validator demo. It is written as a checklist because the full commit -> reveal -> finalize flow depends on local validator state, generated proving artifacts, and the oracle keypair.

For the teammate-facing scripted demo guide, see
[`docs/local-demo-review.md`](local-demo-review.md). For the full scripted path,
run:

```bash
./scripts/local-demo.sh
```

The script writes a timestamped log under `target/local-demo-logs/` and prints
the failing command plus the validator log tail if anything exits nonzero. It
starts a local validator only when one is not already running at
`SOLANA_RPC_URL`. If the script starts the validator, it stops it on exit unless
you run with `KEEP_VALIDATOR=1`. If `ORACLE_VRF_SECRET` is unset or still set
to a placeholder like `0x...`, the script uses a local demo secret.

Before building, the script runs `anchor keys sync` so the compiled
`declare_id!`, `Anchor.toml`, and local deploy keypair all agree. It restores
the source files on exit by default, so this local-only id sync does not stay in
your working tree. This also restores the tracked generated verifying-key
source after local artifact generation. Set `RESTORE_ANCHOR_KEYS=0` if you
intentionally want to keep those local generated source changes. During deploy,
the script also parses the actual program id printed by `anchor deploy` and
uses that id for epoch initialization and oracle transactions.

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

Generate local proving artifacts:

```bash
cargo run -p setup -- local-random
```

Build and deploy in another terminal:

```bash
anchor build --no-idl
anchor deploy
```

## Epoch Setup

Initialize an epoch with the oracle wallet as the authority. First read the
current validator slot:

```bash
CURRENT_SLOT=$(solana slot --url "$SOLANA_RPC_URL")
echo "$CURRENT_SLOT"
```

Choose deadline slots far enough apart to give the oracle process time to
commit, gather entropy, reveal, prove, and finalize. For a local demo, offsets
of roughly 200 / 450 / 750 slots are comfortable:

```bash
cargo run -p oracle --bin oracle -- init-epoch \
  --epoch-id "$EPOCH_ID" \
  --commit-deadline-slot $((CURRENT_SLOT + 200)) \
  --reveal-deadline-slot $((CURRENT_SLOT + 450)) \
  --finalize-deadline-slot $((CURRENT_SLOT + 750))
```

The command prints the `initialize_epoch` signature and the epoch PDA after
confirmation. If the local validator has advanced far past your chosen slots,
rerun `solana slot` and initialize a fresh epoch id with later deadlines.

## Commit And Reveal

Run the oracle:

```bash
cargo run -p oracle --bin oracle -- run
```

Expected behavior:

- The oracle exits early if the epoch PDA is missing.
- During commit phase it submits `oracle_commit` with `SHA256(salt)`.
- It waits for reveal phase.
- It gathers entropy, writes `oracle/archive/<epoch>/`, and submits `oracle_reveal(salt, manifest_hash)`.
- The program verifies the salt commitment and stores `entropy_manifest_hash`, `entropy_seed`, and `aggregated_seed`.

## Finalize

The oracle `run` command continues into finalize phase. It reads the stored
epoch seed, calls the prover with `ORACLE_VRF_SECRET`, checks the generated
public inputs, and submits `finalize_epoch`. The oracle passes the epoch seed to
the prover with `--alpha-hex` so the proof binds to the raw on-chain seed bytes,
not a UTF-8 hex string.

Manual checks after finalization:

- `is_finalized == true`
- `phase == Closed`
- `vrf_output == public_inputs[1]`
- Unauthorized wallets cannot call commit, reveal, or finalize.
- A reveal with the wrong salt fails.

## Known Gaps

- Participant commit/reveal remains optional MVP work.
- A production demo should archive or pin the entropy manifest outside the local filesystem.

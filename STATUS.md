# Random Dungeon — overall project status (May 10, 2026)

## Headline

**MVP is complete and deployed on Solana devnet.** All code, scripts, and
visualizer plumbing are merged into `main`. The on-chain program is live
at the address below; the visualizer's hero badge links straight to its
Solana Explorer page.

| | |
|---|---|
| **Program ID** | [`2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH`](https://explorer.solana.com/address/2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH?cluster=devnet) |
| **Cluster** | devnet |
| **Deploy tx** | [`4x1XfeAJHU1kQF6XJDx8vjdkvthaddMLq4Uivww9nTA3NeBmusQTgiSKsv62FWmxNWSceGjHjL584JSxsRV9AKEk`](https://explorer.solana.com/tx/4x1XfeAJHU1kQF6XJDx8vjdkvthaddMLq4Uivww9nTA3NeBmusQTgiSKsv62FWmxNWSceGjHjL584JSxsRV9AKEk?cluster=devnet) |
| **Deployed at** | 2026-05-10 12:08:15 UTC |
| **Deployer** | `e5hGi4s4aSADChGq3kGbL7n4hWESYW8qgD5cSMzsj3s` |

The exact deploy info is committed in [`web/public/deploy.json`](web/public/deploy.json) and is what the visualizer reads to render the "Deployed · devnet" badge.

This document supersedes earlier status docs.

---

## What works end-to-end today

### On-chain program (`programs/randomness-beacon`)

Deployed. Anchor program is feature-complete for the MVP:

| Instruction | Behavior |
|---|---|
| `initialize_epoch` | creates a per-epoch PDA, registers the oracle pubkey, sets commit/reveal/finalize slot deadlines |
| `oracle_commit` | stores `SHA256(salt)` during commit phase, gated by oracle signer + slot deadline |
| `oracle_reveal` | accepts `(salt, manifest_hash)`, verifies salt commitment, computes `entropy_seed = SHA256("random-dungeon/oracle-seed/v1" \|\| salt \|\| manifest_hash)` and stores it |
| `finalize_epoch` | independently re-derives `alpha_hash = reduce(SHA256(stored entropy_seed))`, requires the proof's first public input matches, runs Groth16 verification, requires `vrf_output == beta` |

The program rejects unauthorized signers, missed deadlines, mismatched
commitments, mismatched `alpha_hash`, and bad proofs. A malicious oracle
cannot submit a proof for a different alpha than the seed it committed to.

### Off-chain oracle (`oracle/`)

Long-running service in `oracle/src/runner.rs`:

- Polls the epoch PDA every 5 s.
- In commit phase: generates salt + sends `oracle_commit(SHA256(salt))`.
- In reveal phase: calls `build_entropy_bundle(epoch)` → archives raw
  responses → submits `oracle_reveal(salt, manifest_hash)`.
- In finalize phase: reads the on-chain `entropy_seed`, calls the prover
  with `--alpha-hex 0x<seed>`, cross-checks `alpha_hash`/`beta` against
  vrf-core's pure computation, submits `finalize_epoch`.
- Has graceful retries, RPC timeouts, signal handling, and unit tests on
  the action-decision logic.

### Entropy module (`oracle/src/entropy/`)

Four sources canonicalized per `docs/entropy.md`: USGS earthquakes, NWS
observations from 5 stations, Bitcoin block hash via blockchain.info (with
mempool.space fallback), drand. Reproducibility is pinned by
`oracle/tests/reproducibility.rs` (51 oracle tests pass).

### SNARK + VRF (`crates/{vrf-core,vrf-circuit,setup,prover,verifier-client,solana-program}`)

BN254 Groth16 pipeline:

- `vrf-core`: native `alpha_hash = SHA256(alpha) → Fr`, `h = Poseidon(alpha_hash)`, `gamma = sk·h`, `beta = Poseidon(gamma)`.
- `vrf-circuit`: R1CS for the same computation.
- `prover`: produces a 256-byte Solana proof (`-A || B || C`) and 64-byte public inputs (`alpha_hash || beta`).
- `verifier-client`: local Arkworks verification.
- `programs/randomness-beacon/src/verifier.rs`: on-chain Groth16
  verification via groth16-solana, **now running on devnet** behind program
  id `2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH`.

### Static visualizer (`web/`)

Single-page no-build site at `python3 -m http.server` from `web/`:

- 4 entropy-source cards (BTC, drand, NWS, USGS) with real headline
  statistics computed in the browser.
- 5-stage pipeline diagram (sources → canonical → manifest → seed → VRF)
  with click-through panels.
- Epoch dropdown (1, 2, 3) with three real archived snapshots.
- Three "verify yourself" buttons running real `crypto.subtle.digest` in
  the browser:
  - `SHA256(domain || "seed" || manifest_hash) == archived seed`
  - `manifest_hash` re-derived from per-source canonical hashes (full
    reproducibility chain)
  - `alpha_hash = SHA256(epoch_seed)` reduced mod r == published `alpha_hash`
- **On-chain deployment block + hero badge** that read
  `web/public/deploy.json` and link to the live program on Solana
  Explorer (badge: "Deployed · devnet").

### Demo + tooling

- `scripts/local-demo.sh` — end-to-end local validator demo (validator →
  deploy → init epoch → commit → reveal → finalize).
- `scripts/deploy-devnet.sh` — idempotent devnet deploy script. Already
  used to land the live program; rerun it to push an upgrade.
- `docs/local-demo-review.md` and `docs/demo-walkthrough.md` cover manual
  + scripted local demo paths.
- `docs/deploy-devnet-guide.md` — step-by-step for the manually-funded
  faucet path used to ship the live deploy.

---

## What's not done — stretch goals deferred (not MVP-blocking)

Per the slack thread consensus and the README's milestone list:

- **Participant commit/reveal:** deferred. The README explicitly lists
  this as stretch. Single-party oracle is fine for course MVP.
- **Real trusted-setup ceremony:** deferred. `local-random` is dev-only;
  the README's Security Considerations section already calls this out.
  Powers of Tau + circuit-specific phase 2 would be needed for production
  but not for MVP demo.
- **Full RFC 9381 ECVRF:** deferred. Current MVP proves the scalar-field
  shortcut `gamma = sk·h, beta = Poseidon(gamma)`; full ECVRF would
  require hash-to-curve in-circuit.
- **Durable archive (IPFS/Arweave) for manifests:** deferred. Currently
  `oracle/archive/<epoch>/` writes to local filesystem; auditors need
  access to the archive directly.
- **Anchor 0.31+ upgrade** to drop the `--no-idl` workaround: deferred.
- **Mainnet deploy:** out of scope for this MVP. Would require a real
  ceremony plus a separate deploy keypair and SOL.

---

## What's left for the writeup / submission

These are non-code tasks for finishing the deliverable:

1. **(Optional, recommended)** Run `scripts/local-demo.sh` and capture
   the commit→reveal→finalize cycle. Screenshot the visualizer's hero
   badge and the Explorer page for the program account.
2. **(Optional)** Run a finalize round against the *deployed* devnet
   program (`SOLANA_RPC_URL=https://api.devnet.solana.com cargo run -p
   oracle --bin oracle -- run` with appropriate env per
   `docs/demo-walkthrough.md`) to demonstrate end-to-end execution on a
   real cluster. This produces an on-chain finalized epoch the writeup
   can link to.
3. **Writeup.** Reference the program-id Explorer link above, the
   architecture in this doc, the live visualizer, and the test
   coverage (`cargo test -p oracle --lib` + the prover/verifier crate
   tests).

Everything else for MVP is done, deployed, and tested.

---

## Branches and PRs (history)

- `main` — latest is `c76db46` ("deployed on devnet"). Has the full
  end-to-end deploy.
- `mvp-final-prep` — feature integration branch; merged into `main` via
  PR #10 ahead of the deploy commit.
- PRs #5, #7, #8, #9 — visualizer + deploy-script + audit-fix PRs,
  all merged.

## Reproducibility

To reproduce the deploy from a fresh checkout:

```bash
git clone https://github.com/IanKammerman/Random-Dungeon
cd Random-Dungeon
# follow docs/deploy-devnet-guide.md, OR:
SKIP_AIRDROP=1 scripts/deploy-devnet.sh   # after funding $(solana address)
```

Note: `anchor deploy` will publish at *your* deploy keypair's address,
which differs from `2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH` unless
you ship the same keypair. The deploy script handles `anchor keys sync`
automatically and writes the new program id into
`web/public/deploy.json`.

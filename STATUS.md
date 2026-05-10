# Random Dungeon — overall project status (May 10, 2026)

## Headline

**MVP is feature-complete and tested. The only remaining task is a
~15-minute manual ops step to deploy the program to Solana devnet.** All
code, scripts, and visualizer plumbing for that deploy already exist and
are validated; the blocker is purely getting devnet SOL into a deploy
wallet via the public faucet, which is free but rate-limited from shared
network IPs.

This document supersedes the May 9 status doc.

---

## What works end-to-end today

### On-chain program (`programs/randomness-beacon`)

The Anchor program is feature-complete for the MVP. It implements:

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
  verification via groth16-solana.

### Static visualizer (`web/`) — merged via PR #5, extended by PR #8

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
  - **(PR #8)** `alpha_hash = SHA256(epoch_seed)` reduced mod r ==
    published `alpha_hash`
- **(PR #8)** On-chain deployment block + hero badge that read
  `web/public/deploy.json` (program id + Solana Explorer link, or
  "deploy pending" status).

### Demo + tooling

- `scripts/local-demo.sh` — end-to-end local validator demo (validator →
  deploy → init epoch → commit → reveal → finalize), already on `main`.
- `docs/local-demo-review.md` and `docs/demo-walkthrough.md` cover manual
  + scripted demo paths.
- **(PR #8)** `scripts/deploy-devnet.sh` — idempotent devnet deploy script
  that mirrors local-demo's structure but targets devnet, with airdrop
  fallback and writes a populated `web/public/deploy.json` for the
  visualizer.

---

## What's not done — and why

### Devnet deployment ⚠️ blocked on a funded wallet (only remaining MVP gap)

The script (`scripts/deploy-devnet.sh`) is written, syntax-checked, and the
build/key-resolution path is validated:

- `anchor build --no-idl` produces `target/deploy/randomness_beacon.so` (234 KB).
- `anchor keys list` resolves `randomness_beacon: 5MMjTfc64Q9AC2rjVda1ZHH137TebNpdUzNhTMg7Vypx`.
- That program id will be the deployed id (anchor uses the deploy keypair
  pubkey).

**The blocker:** devnet airdrop is rate-limited from the network the
previous attempt ran from. `solana airdrop` returned `airdrop request
failed` on `api.devnet.solana.com`, `devnet.solana.com`, and Ankr (auth
required).

**Decision: stay on devnet, do not switch to testnet.** Testnet is
primarily for Solana validator/protocol-upgrade testing, gets reset
aggressively, and has the same kind of faucet rate limiting. The
"deployed on devnet" Explorer link is the convention graders/audiences
expect for a Solana smart-contract demo.

**Resolution path (Path 1 — recommended):**

1. A teammate signs in to https://faucet.solana.com/ with a GitHub account
   that has some history. The signed-in limit is much higher than
   anonymous.
2. They paste the deploy wallet pubkey (`solana address`) and request 2
   SOL. Repeat until balance ≥ 4 SOL.
3. They run `SKIP_AIRDROP=1 scripts/deploy-devnet.sh` from a checkout of
   `mvp-final-prep`.
4. They commit the resulting `web/public/deploy.json`.

**Backup paths if Path 1 stalls:**

- **Alternate faucets:** DevnetFaucet.org (separate rate-limit pool), or
  QuickNode/Alchemy free-tier faucets which require a free dev account
  signup.
- **Personal RPC for the airdrop:** sign up for a free Helius or QuickNode
  devnet endpoint and run the airdrop against that URL via
  `solana config set --url <url>`. Bypasses the shared-IP rate-limit
  bucket.
- **Use a pre-funded wallet:** if anyone already has devnet SOL in an old
  keypair, run with
  `DEPLOY_KEYPAIR_PATH=/path/to/funded-keypair.json SKIP_AIRDROP=1 scripts/deploy-devnet.sh`.

**Cost: $0 across all paths.** Devnet SOL has no monetary value and all
faucets/RPCs listed above are free for this volume of use.

Once deployed, the script auto-writes `web/public/deploy.json` with the
program id + explorer URL, and the visualizer's hero badge flips from
"Devnet deploy pending" to a clickable "Deployed · devnet" link.

See `docs/deploy-devnet-guide.md` (or the standalone teammate guide) for
the step-by-step.

### Stretch goals deferred (not MVP-blocking)

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

---

## Branches and PRs right now

- `main` — latest is `ee749f1`. Has the full local-validator demo working
  end-to-end.
- `mvp-final-prep` — branch created from `main`, target for the current
  iteration.
- `claude/sleepy-wescoff-eb67a3` — feature branch with the three MVP
  fixes (deploy script, alpha_hash binding, refreshed SNARK).
- **PR #8** → targets `mvp-final-prep`. Contains the deploy script, the
  alpha_hash binding check, the deploy-info section in the visualizer,
  and the regenerated SNARK proof.
- PR #7 was the same content but targeted at `main`; closed. Commits are
  preserved in PR #8 against `mvp-final-prep`.

---

## What ships the MVP from here

1. **(Code-side, optional)** Run the Claude Code sanity-check prompt to
   validate `scripts/deploy-devnet.sh` defaults (especially that
   `MIN_DEVNET_SOL` is 4, not 2, and that `SKIP_AIRDROP=1` is honored).
   If it makes any fixes, merge them into `mvp-final-prep`.
2. **(Ops, ~15–25 min)** A teammate fund the deploy wallet via
   https://faucet.solana.com/ (GitHub login → 2× 2 SOL → ≥ 4 SOL total),
   then runs `SKIP_AIRDROP=1 scripts/deploy-devnet.sh`. Full step-by-step
   in the teammate deployment guide.
3. **(Ops, 1 min)** Commit the resulting `web/public/deploy.json` so the
   visualizer's "Deployed · devnet" badge is live.
4. **(Optional)** Run `scripts/local-demo.sh` and screenshot/record the
   commit→reveal→finalize cycle for the writeup.
5. **(Final)** Merge `mvp-final-prep` → `main`.

Everything else for MVP is done and tested.

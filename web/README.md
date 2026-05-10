# Random Dungeon — visualizer

Static-site visualizer for the Random Dungeon randomness beacon. Plain HTML +
CSS + ES-module JS, no build step. Serve from this directory:

```bash
python3 -m http.server 8000
# open http://localhost:8000
```

## What it shows

- The four entropy sources (USGS, NWS, BTC, drand) for each archived epoch
  with real headline statistics computed in the browser.
- A five-stage pipeline diagram (sources → canonical → manifest → seed → VRF)
  with click-through panels showing the actual bytes for the selected epoch.
- Three "verify yourself" buttons that run real `crypto.subtle.digest` calls
  in the browser:
    1. `SHA256(domain || "seed" || manifest_hash) == archived seed?`
    2. `manifest_hash` re-derived from per-source canonical hashes by porting
       the manifest layout from `oracle/src/entropy/manifest.rs` to JS — full
       reproducibility chain per `docs/entropy.md`.
    3. `alpha_hash = SHA256(epoch_seed) reduced mod r == published alpha_hash?`
       — binds the displayed Groth16 proof to a real archived epoch.
- An on-chain deployment block that reads `web/public/deploy.json` and shows
  the program id + cluster + Solana Explorer link once the program is deployed.

## Files served

```
web/
  index.html, styles.css, main.js, lib/*.js
  public/
    archives/{1,2,3}/{btc,drand,nws,usgs,manifest}.json
    snark/public_inputs.json   ← Groth16 public inputs [alpha_hash, beta]
    snark/snark_meta.json      ← which epoch's seed was used as alpha
    deploy.json                ← devnet deploy info (placeholder until deploy)
```

## Refreshing archive data

To capture a new epoch (or refresh existing ones), run the `entropy_once`
binary from the repo root, then copy the output into this directory:

```bash
cargo run -p oracle --bin entropy_once 4
cp -r oracle/archive/4 web/public/archives/4
```

`entropy_once <N>` fetches the four entropy sources, canonicalizes them,
builds a manifest, and writes
`oracle/archive/<N>/{btc,drand,nws,usgs,manifest}.json`. The enriched
`manifest.json` includes per-source canonical hashes so the in-browser
"Recompute manifest_hash" button can re-derive the manifest hash without
re-canonicalizing the raw JSON. After copying a new epoch, append its number
to the `listEpochs()` array in `lib/archive.js`.

## Refreshing the SNARK proof

The displayed Groth16 proof is generated from the named epoch's audit seed
so the in-browser `alpha_hash` binding check passes. To regenerate (e.g.
after adding a new epoch):

```bash
SEED=$(jq -r .seed web/public/archives/3/manifest.json)
cargo run -p prover --release -- --sk 12345 --alpha-hex 0x$SEED
cp artifacts/public_inputs.json web/public/snark/public_inputs.json
# update the alpha_epoch field in web/public/snark/snark_meta.json to match
```

The proof is locally verifiable with `cargo run -p verifier-client --release`.

## Devnet deployment

The visualizer reads `web/public/deploy.json` to render the program id +
explorer link. The committed file is a placeholder; populate it by running:

```bash
scripts/deploy-devnet.sh
```

The script configures solana CLI for devnet, ensures the deploy wallet is
funded (with airdrop fallback), generates the Groth16 verifying-key constants,
builds the Anchor program, deploys it, and writes a populated `deploy.json`
into this directory. See the script's header comment for environment
variables and exit codes. After a successful deploy, refresh the static site
and the hero badge will switch from "Devnet deploy pending" to a clickable
"Deployed · devnet" link.

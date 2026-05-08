# Random Dungeon — visualizer

Static-site visualizer for the Random Dungeon randomness beacon. Plain HTML +
CSS + ES-module JS, no build step. Serve from this directory:

```bash
python3 -m http.server 8000
# open http://localhost:8000
```

## Refreshing archive data

Pre-computed epoch snapshots live under `web/public/archives/<epoch>/` and are
checked in. To capture a new epoch (or refresh existing ones), run the
`entropy_once` binary from the repo root, then copy the output into this
directory:

```bash
cargo run -p oracle --bin entropy_once 4
cp -r oracle/archive/4 web/public/archives/4
```

`entropy_once <N>` fetches the four entropy sources (USGS, NWS, BTC, drand),
canonicalizes them, builds a manifest, and writes
`oracle/archive/<N>/{btc,drand,nws,usgs,manifest}.json`. The enriched
`manifest.json` includes per-source canonical hashes so the in-browser
"Recompute manifest_hash" button can re-derive the manifest hash without
re-canonicalizing the raw JSON. After copying a new epoch, append its number
to the `listEpochs()` array in `lib/archive.js`.

The SNARK public inputs come from `artifacts/public_inputs.json` at the repo
root and are mirrored to `web/public/snark/public_inputs.json`. Re-copy after
each `cargo run -p prover` invocation.

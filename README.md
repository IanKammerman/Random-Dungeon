#Random Dungeon
# Verifiable Randomness Beacon on Solana

A publicly verifiable randomness beacon on Solana.

**Team:** Tony Fields, Ian Kammerman, Cathy Zhang
**Course:** COMS 4995 — Sciences of Blockchain

## Overview

TODO: one-paragraph description of what the beacon does and who it's for.

## Design

TODO: high-level description of how the system works end-to-end.

### Architecture

TODO: on-chain program, off-chain oracle, contribution protocol.

The off-chain oracle service submits entropy seeds to the Solana program via a Helius RPC endpoint, which provides reliable transaction submission and devnet/mainnet access without running a full validator. The RPC URL is configured via the `SOLANA_RPC_URL` environment variable so providers can be swapped without code changes.

### Entropy Sources

NASA API: Sun spots cant be controlled by any person, and its through NASA which is a reliable source.
Weather API (NOAA/weather.gov API): Hard to manipulate, random and unpredictable.


### VRF

TODO: which VRF, keying model.

### Contribution Protocol

TODO: commit-reveal spec, non-revealer fallback.

### Epoch Timing

TODO: epoch length, commit window, reveal window.

### On-Chain Storage

TODO: what's stored per epoch.

## Milestones

### MVP

TODO

### Realistic Target

TODO

### Stretch Target

TODO

## Security Considerations

TODO

## Why Solana

TODO

## Related Work

TODO

## Getting Started

TODO: build and run instructions.

## License

TODO
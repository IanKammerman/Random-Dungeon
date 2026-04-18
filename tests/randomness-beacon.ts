// tests/randomness-beacon.ts
//
// TODO: Anchor integration tests for the randomness-beacon program.
//
// Planned coverage:
//   - program initialization / config account setup
//   - commit phase: oracle submits hashed entropy commitments
//   - reveal phase: oracle reveals preimages, program verifies hashes
//   - finalize phase: program derives and stores the epoch's beacon output
//   - failure paths: late reveals, mismatched preimages, unauthorized signers
//
// Uses @coral-xyz/anchor, @solana/web3.js, mocha, and chai (to be added later).

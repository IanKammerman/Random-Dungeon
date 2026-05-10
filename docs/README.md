# docs/

Project documentation lives here.

- **local-demo-review.md** — teammate-facing guide for running the full local demo, checking success output, and understanding what the script does.
- **demo-walkthrough.md** — manual local validator checklist for deploy, epoch setup, commit/reveal, and finalize.
- **deploy-devnet-guide.md** — step-by-step for deploying `randomness_beacon` to Solana devnet via the manually-funded faucet path. Pairs with `scripts/deploy-devnet.sh`.
- **snark-vrf-integration.md** — Groth16/VRF prover and verifier integration notes.
- **entropy.md** — entropy source policy, canonicalization, archive format, and seed derivation.

For the current overall project status, see [`STATUS.md`](../STATUS.md) at the repo root.

Still useful future docs:

- **Architecture** — high-level overview of the on-chain program and off-chain oracle, their interactions, and data flow.
- **Protocol spec** — formal description of the commit/reveal/finalize protocol, account layouts, and PDAs.
- **Threat model** — assumptions, adversaries considered, and mitigations.
- **Deployment docs** — runbooks for devnet/mainnet deployment, key management, and oracle operations.

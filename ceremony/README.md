# Powers of Tau / MPC Ceremony

The Rust `local-random` setup is for development only. Production Groth16 deployments need a ceremony so no single participant knows the toxic waste.

## Commands

```bash
# Install snarkjs and circomlib
npm install -g snarkjs
npm install circomlib

# Compile circuit
circom ceremony/vrf.circom --r1cs --wasm --sym -o build/

# Start phase 1
snarkjs powersoftau new bn128 14 ceremony/ptau/pot14_0000.ptau -v

# First contribution
snarkjs powersoftau contribute \
  ceremony/ptau/pot14_0000.ptau \
  ceremony/ptau/pot14_0001.ptau \
  --name="First contribution" \
  -v

# Optional: more participants repeat contribute

# Prepare phase 2
snarkjs powersoftau prepare phase2 \
  ceremony/ptau/pot14_0001.ptau \
  ceremony/ptau/pot14_final.ptau \
  -v

# Circuit-specific Groth16 setup
snarkjs groth16 setup \
  build/vrf.r1cs \
  ceremony/ptau/pot14_final.ptau \
  ceremony/zkey/vrf_0000.zkey

# Phase 2 contribution
snarkjs zkey contribute \
  ceremony/zkey/vrf_0000.zkey \
  ceremony/zkey/vrf_0001.zkey \
  --name="phase2 contributor 1" \
  -v

# Export verification key
snarkjs zkey export verificationkey \
  ceremony/zkey/vrf_0001.zkey \
  artifacts/verifying_key.json
```

Import the ceremony output into the Rust artifact path:

```bash
cargo run -p setup -- import-ceremony \
  --zkey ceremony/zkey/vrf_0001.zkey \
  --vk-json artifacts/verifying_key.json
```

The Powers of Tau phase can be reused across circuits up to the supported circuit size. The Groth16 phase 2 setup is circuit-specific and must be rerun whenever the circuit changes.

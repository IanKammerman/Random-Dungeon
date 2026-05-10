#!/usr/bin/env bash
#
# Deploy the randomness-beacon Anchor program to Solana devnet and capture
# the deployed program id for the visualizer.
#
# Outputs:
#   web/public/deploy.json — read by the static visualizer to render
#       "Deployed at: <program id> on devnet" with an explorer link.
#
# Idempotent: rerunning re-deploys (anchor deploys an upgrade if the program
# already exists at the same address). The program id stays stable across
# deploys because anchor uses the deploy keypair's pubkey as the program id.
#
# Required env (with sensible defaults):
#   SOLANA_RPC_URL   default https://api.devnet.solana.com
#   DEPLOY_KEYPAIR_PATH  default ~/.config/solana/id.json (the wallet that
#                        pays for the deploy and owns the program upgrade
#                        authority)
#   ANCHOR_BUILD_OPTS    extra flags for `anchor build` (default: --no-idl)
#   MIN_DEVNET_SOL       minimum SOL balance before deploying (default 4).
#                        Rule of thumb for a ~234 KB program: ~3.5 SOL
#                        rent-exempt for the program account + ~0.5 SOL
#                        slack for tx fees and the buffer account. Devnet
#                        airdrops cap at 2 SOL per request, so two
#                        airdrops (or two faucet hits) get you there.
#   SKIP_AIRDROP         set to 1 to skip the airdrop attempt.
#   SKIP_BUILD           set to 1 to reuse target/deploy/*.so from a prior
#                        build (faster reruns).
#
# Exit codes:
#   0  deploy succeeded; web/public/deploy.json updated
#   1  prerequisite missing (toolchain, keypair, etc.)
#   2  airdrop and balance check failed (devnet rate-limited)
#   3  build failed
#   4  deploy failed
#   5  could not parse program id from deploy output

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

LOG_DIR="${LOG_DIR:-$ROOT_DIR/target/deploy-devnet-logs}"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/deploy-devnet-$(date +%Y%m%d-%H%M%S).log"
DEPLOY_LOG="$LOG_DIR/anchor-deploy.log"
exec > >(tee -a "$LOG_FILE") 2>&1

SOLANA_RPC_URL="${SOLANA_RPC_URL:-https://api.devnet.solana.com}"
DEPLOY_KEYPAIR_PATH="${DEPLOY_KEYPAIR_PATH:-$HOME/.config/solana/id.json}"
ANCHOR_BUILD_OPTS="${ANCHOR_BUILD_OPTS:---no-idl}"
MIN_DEVNET_SOL="${MIN_DEVNET_SOL:-4}"
SKIP_AIRDROP="${SKIP_AIRDROP:-0}"
SKIP_BUILD="${SKIP_BUILD:-0}"

step() { echo; echo "==> $*"; }
fail() { echo; echo "[FAIL] $*"; echo "[FAIL] Full log: $LOG_FILE"; exit "${2:-1}"; }
require_cmd() { command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"; }

trap 'echo; echo "[FAIL] Command failed: $BASH_COMMAND"; echo "[FAIL] Full log: $LOG_FILE"' ERR

step "Checking prerequisites"
require_cmd cargo
require_cmd solana
require_cmd solana-keygen
require_cmd anchor

if [[ ! -f "$DEPLOY_KEYPAIR_PATH" ]]; then
  step "Creating deploy wallet at $DEPLOY_KEYPAIR_PATH"
  mkdir -p "$(dirname "$DEPLOY_KEYPAIR_PATH")"
  solana-keygen new --outfile "$DEPLOY_KEYPAIR_PATH" --no-bip39-passphrase --silent
fi

DEPLOY_PUBKEY="$(solana address --keypair "$DEPLOY_KEYPAIR_PATH")"
echo "[INFO] SOLANA_RPC_URL  = $SOLANA_RPC_URL"
echo "[INFO] DEPLOY_PUBKEY   = $DEPLOY_PUBKEY"
echo "[INFO] DEPLOY_KEYPAIR  = $DEPLOY_KEYPAIR_PATH"

step "Configuring solana CLI for devnet"
solana config set --url "$SOLANA_RPC_URL" --keypair "$DEPLOY_KEYPAIR_PATH" >/dev/null

step "Checking devnet balance for $DEPLOY_PUBKEY"
balance_sol() {
  # Returns the float balance in SOL, or 0 if RPC is unreachable.
  solana balance "$DEPLOY_PUBKEY" --url "$SOLANA_RPC_URL" 2>/dev/null \
    | awk '{print $1+0}' \
    || echo 0
}
current_balance="$(balance_sol)"
echo "[INFO] Current balance: ${current_balance} SOL (need >= ${MIN_DEVNET_SOL})"

if (( $(echo "$current_balance < $MIN_DEVNET_SOL" | bc -l) )); then
  if [[ "$SKIP_AIRDROP" == "1" ]]; then
    cat <<EOF >&2

[FAIL] Deploy wallet is underfunded and SKIP_AIRDROP=1 was set.
       wallet pubkey   : $DEPLOY_PUBKEY
       current balance : ${current_balance} SOL
       required        : ${MIN_DEVNET_SOL} SOL (set MIN_DEVNET_SOL=N to override)

       To top up:
         1. Open https://faucet.solana.com/ and sign in with GitHub.
         2. Paste $DEPLOY_PUBKEY into the address field, request 2 SOL.
         3. Repeat until 'solana balance --url $SOLANA_RPC_URL' shows >= ${MIN_DEVNET_SOL} SOL.
         4. Re-run: SKIP_AIRDROP=1 scripts/deploy-devnet.sh

       Alternatives if the faucet is rate-limited:
         - Backup faucet: https://faucet.quicknode.com/solana/devnet
         - Use a pre-funded keypair:
             DEPLOY_KEYPAIR_PATH=/path/to/funded.json SKIP_AIRDROP=1 scripts/deploy-devnet.sh
EOF
    fail "Insufficient devnet SOL on $DEPLOY_PUBKEY (have ${current_balance}, need ${MIN_DEVNET_SOL})" 2
  fi
  step "Airdropping SOL on devnet (devnet caps at 2 SOL per request)"
  airdrop_attempts=0
  while (( $(echo "$(balance_sol) < $MIN_DEVNET_SOL" | bc -l) )); do
    airdrop_attempts=$((airdrop_attempts + 1))
    if (( airdrop_attempts > 4 )); then
      cat <<EOF >&2

[FAIL] Could not airdrop enough SOL on devnet after $airdrop_attempts attempts.
       Devnet airdrop is rate-limited and frequently unavailable. Options:
         - retry later: solana airdrop 2 $DEPLOY_PUBKEY --url $SOLANA_RPC_URL
         - use the public faucet: https://faucet.solana.com/
         - or skip with SKIP_AIRDROP=1 if the wallet is already funded.
EOF
      fail "Insufficient devnet SOL after airdrop attempts" 2
    fi
    if ! solana airdrop 2 "$DEPLOY_PUBKEY" --url "$SOLANA_RPC_URL" 2>&1 | tee -a "$LOG_FILE"; then
      echo "[WARN] airdrop attempt $airdrop_attempts failed; retrying in 15s"
      sleep 15
    fi
    sleep 3
  done
fi

step "Ensuring Groth16 proving + verifying artifacts exist"
# `cargo run -p setup -- local-random` regenerates a fresh trusted setup
# and overwrites artifacts/verifying_key_solana.rs. If we did that on every
# run, an "upgrade" deploy would publish a program with different VK
# constants than the one being upgraded — silently invalidating any proofs
# the oracle had already produced against the old VK. So: on re-runs we
# reuse the existing artifacts. To force a fresh setup, set FORCE_SETUP=1
# (typically only when the circuit itself has changed).
if [[ "${FORCE_SETUP:-0}" == "1" ]] \
   || [[ ! -f "$ROOT_DIR/artifacts/proving_key.bin" ]] \
   || [[ ! -f "$ROOT_DIR/artifacts/verifying_key.bin" ]] \
   || [[ ! -f "$ROOT_DIR/artifacts/verifying_key_solana.rs" ]]; then
  echo "[INFO] Generating fresh Groth16 artifacts (local-random)"
  echo "[INFO] local-random is dev-only — see README 'Security Considerations'"
  cargo run -p setup -- local-random
else
  echo "[INFO] Reusing existing artifacts/{proving_key.bin,verifying_key.bin,verifying_key_solana.rs}"
  echo "[INFO] Set FORCE_SETUP=1 to regenerate (rotates the on-chain verifying key)"
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
  step "Building Anchor program ($ANCHOR_BUILD_OPTS)"
  # `anchor keys sync` aligns declare_id! in lib.rs and Anchor.toml with
  # whatever target/deploy/randomness_beacon-keypair.json holds. Doing it
  # here means a fresh checkout (which has no deploy keypair yet) gets a
  # new program id rather than colliding with the placeholder.
  anchor keys sync >/dev/null
  # shellcheck disable=SC2086
  anchor build $ANCHOR_BUILD_OPTS || fail "anchor build failed" 3
fi

step "Resolving program id from anchor keys list"
PROGRAM_ID="$(anchor keys list | awk '/^randomness_beacon:/ {print $2}' | tail -n 1)"
if [[ -z "$PROGRAM_ID" ]]; then
  fail "Could not resolve program id from anchor keys list" 5
fi
echo "[INFO] PROGRAM_ID = $PROGRAM_ID"

step "Deploying randomness-beacon to devnet"
# `anchor deploy` writes the program id and the deploy tx signature to
# stdout, which we tee to $DEPLOY_LOG for parsing.
anchor deploy \
  --provider.cluster "$SOLANA_RPC_URL" \
  --provider.wallet "$DEPLOY_KEYPAIR_PATH" \
  2>&1 | tee "$DEPLOY_LOG" \
  || fail "anchor deploy failed" 4

DEPLOYED_PROGRAM_ID="$(awk '/Program Id:/ {print $3}' "$DEPLOY_LOG" | tail -n 1)"
DEPLOY_SIG="$(awk '/Signature:/ {print $2}' "$DEPLOY_LOG" | tail -n 1)"

if [[ -z "$DEPLOYED_PROGRAM_ID" ]]; then
  fail "Could not parse deployed program id from $DEPLOY_LOG" 5
fi
if [[ "$DEPLOYED_PROGRAM_ID" != "$PROGRAM_ID" ]]; then
  echo "[WARN] anchor keys list said $PROGRAM_ID, but anchor deployed $DEPLOYED_PROGRAM_ID. Using deployed id."
  PROGRAM_ID="$DEPLOYED_PROGRAM_ID"
fi

DEPLOY_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
EXPLORER_PROGRAM_URL="https://explorer.solana.com/address/${PROGRAM_ID}?cluster=devnet"
EXPLORER_TX_URL=""
if [[ -n "$DEPLOY_SIG" ]]; then
  EXPLORER_TX_URL="https://explorer.solana.com/tx/${DEPLOY_SIG}?cluster=devnet"
fi

step "Writing web/public/deploy.json for the visualizer"
mkdir -p "$ROOT_DIR/web/public"
# Field naming: `explorer_url` and `deploy_tx` are the canonical short
# names; `explorer_program_url`, `explorer_tx_url`, and `deploy_signature`
# are kept as aliases the existing visualizer reads. Re-running this
# script overwrites the file with a fresh `deployed_at` and `deploy_tx`,
# which is what `idempotent` means here.
cat > "$ROOT_DIR/web/public/deploy.json" <<EOF
{
  "program_id": "${PROGRAM_ID}",
  "cluster": "devnet",
  "rpc_url": "${SOLANA_RPC_URL}",
  "deploy_tx": "${DEPLOY_SIG}",
  "deploy_signature": "${DEPLOY_SIG}",
  "deployed_at": "${DEPLOY_TIMESTAMP}",
  "deployer": "${DEPLOY_PUBKEY}",
  "explorer_url": "${EXPLORER_PROGRAM_URL}",
  "explorer_program_url": "${EXPLORER_PROGRAM_URL}",
  "explorer_tx_url": "${EXPLORER_TX_URL}",
  "status": "deployed"
}
EOF
echo "[OK] Wrote web/public/deploy.json"

step "Done"
echo "[OK] Program id:        $PROGRAM_ID"
echo "[OK] Cluster:           devnet"
echo "[OK] Explorer (program): $EXPLORER_PROGRAM_URL"
[[ -n "$EXPLORER_TX_URL" ]] && echo "[OK] Explorer (tx):      $EXPLORER_TX_URL"
echo "[OK] Full log:          $LOG_FILE"

#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

LOG_DIR="${LOG_DIR:-$ROOT_DIR/target/local-demo-logs}"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/local-demo-$(date +%Y%m%d-%H%M%S).log"
VALIDATOR_LOG="$LOG_DIR/validator.log"
DEPLOY_LOG="$LOG_DIR/anchor-deploy.log"

exec > >(tee -a "$LOG_FILE") 2>&1

STARTED_VALIDATOR=0
VALIDATOR_PID=""
SYNCED_ANCHOR_KEYS=0
SOURCE_BACKUP_DIR=""

fail() {
  echo
  echo "[FAIL] $*"
  echo "[FAIL] Full log: $LOG_FILE"
  if [[ -n "${VALIDATOR_LOG:-}" && -f "$VALIDATOR_LOG" ]]; then
    echo "[FAIL] Validator log tail:"
    tail -n 80 "$VALIDATOR_LOG" || true
  fi
  exit 1
}

on_error() {
  local status=$?
  echo
  echo "[FAIL] Command failed with exit code $status:"
  echo "       $BASH_COMMAND"
  echo "[FAIL] Full log: $LOG_FILE"
  if [[ -n "${VALIDATOR_LOG:-}" && -f "$VALIDATOR_LOG" ]]; then
    echo "[FAIL] Validator log tail:"
    tail -n 80 "$VALIDATOR_LOG" || true
  fi
  exit "$status"
}

cleanup() {
  if [[ -n "$SOURCE_BACKUP_DIR" && "${RESTORE_ANCHOR_KEYS:-1}" == "1" ]]; then
    echo
    echo "[INFO] Restoring source files after local demo"
    cp "$SOURCE_BACKUP_DIR/Anchor.toml" "$ROOT_DIR/Anchor.toml" || true
    cp "$SOURCE_BACKUP_DIR/lib.rs" "$ROOT_DIR/programs/randomness-beacon/src/lib.rs" || true
    if [[ -f "$SOURCE_BACKUP_DIR/verifying_key_solana.rs" ]]; then
      cp "$SOURCE_BACKUP_DIR/verifying_key_solana.rs" "$ROOT_DIR/artifacts/verifying_key_solana.rs" || true
    fi
  fi
  if [[ "$STARTED_VALIDATOR" == "1" && "${KEEP_VALIDATOR:-0}" != "1" ]]; then
    echo
    echo "[INFO] Stopping local validator pid $VALIDATOR_PID"
    kill "$VALIDATOR_PID" >/dev/null 2>&1 || true
  fi
}

trap on_error ERR
trap cleanup EXIT

step() {
  echo
  echo "==> $*"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

wait_for_validator() {
  local attempts="${1:-60}"
  for _ in $(seq 1 "$attempts"); do
    if solana slot --url "$SOLANA_RPC_URL" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  fail "Timed out waiting for local validator at $SOLANA_RPC_URL"
}

ensure_wallet() {
  if [[ ! -f "$ORACLE_KEYPAIR_PATH" ]]; then
    step "Creating local oracle wallet at $ORACLE_KEYPAIR_PATH"
    mkdir -p "$(dirname "$ORACLE_KEYPAIR_PATH")"
    solana-keygen new \
      --outfile "$ORACLE_KEYPAIR_PATH" \
      --no-bip39-passphrase
  fi
}

set_demo_vrf_secret() {
  local default_secret="0x0000000000000000000000000000000000000000000000000000000000003039"
  local configured_secret="${ORACLE_VRF_SECRET:-}"

  if [[ -z "$configured_secret" || "$configured_secret" == "0x..." || "$configured_secret" == "..." ]]; then
    export ORACLE_VRF_SECRET="$default_secret"
    echo "[INFO] Using local demo ORACLE_VRF_SECRET"
    return
  fi

  local hex_secret="${configured_secret#0x}"
  if [[ -z "$hex_secret" || ! "$hex_secret" =~ ^[0-9a-fA-F]+$ ]]; then
    fail "ORACLE_VRF_SECRET must be a hex BN254 scalar such as $default_secret. Unset ORACLE_VRF_SECRET to use the demo default."
  fi
  if (( ${#hex_secret} % 2 != 0 )); then
    fail "ORACLE_VRF_SECRET has an odd number of hex digits. Add a leading 0 nibble, or unset ORACLE_VRF_SECRET to use the demo default."
  fi
  if (( ${#hex_secret} > 64 )); then
    fail "ORACLE_VRF_SECRET is longer than 32 bytes. Use a smaller hex scalar, or unset ORACLE_VRF_SECRET to use the demo default."
  fi

  export ORACLE_VRF_SECRET="$configured_secret"
}

backup_source_files() {
  SOURCE_BACKUP_DIR="$(mktemp -d "$LOG_DIR/source-backup.XXXXXX")"
  cp "$ROOT_DIR/Anchor.toml" "$SOURCE_BACKUP_DIR/Anchor.toml"
  cp "$ROOT_DIR/programs/randomness-beacon/src/lib.rs" "$SOURCE_BACKUP_DIR/lib.rs"
  if [[ -f "$ROOT_DIR/artifacts/verifying_key_solana.rs" ]]; then
    cp "$ROOT_DIR/artifacts/verifying_key_solana.rs" "$SOURCE_BACKUP_DIR/verifying_key_solana.rs"
  fi
}

sync_anchor_keys_for_local_deploy() {
  step "Syncing Anchor program id to the local deploy keypair"
  SYNCED_ANCHOR_KEYS=1

  echo "[INFO] Before sync:"
  anchor keys list
  anchor keys sync

  local synced_program_id
  synced_program_id="$(anchor keys list | awk '/^randomness_beacon:/ {print $2}' | tail -n 1)"
  if [[ -z "$synced_program_id" ]]; then
    fail "Could not determine randomness_beacon program id from anchor keys list"
  fi
  export PROGRAM_ID="$synced_program_id"
  echo "[INFO] Active PROGRAM_ID after key sync: $PROGRAM_ID"
  echo "[INFO] Source files will be restored on exit. Set RESTORE_ANCHOR_KEYS=0 to keep local generated source changes."
}

start_validator_if_needed() {
  if solana slot --url "$SOLANA_RPC_URL" >/dev/null 2>&1; then
    echo "[INFO] Reusing validator already running at $SOLANA_RPC_URL"
    return 0
  fi

  step "Starting local validator"
  mkdir -p "$VALIDATOR_LEDGER_DIR"
  solana-test-validator \
    --ledger "$VALIDATOR_LEDGER_DIR" \
    --reset \
    > "$VALIDATOR_LOG" 2>&1 &
  VALIDATOR_PID="$!"
  STARTED_VALIDATOR=1
  echo "[INFO] Validator pid: $VALIDATOR_PID"
  echo "[INFO] Validator log: $VALIDATOR_LOG"
  wait_for_validator 90
}

export SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://localhost:8899}"
export ORACLE_KEYPAIR_PATH="${ORACLE_KEYPAIR_PATH:-$HOME/.config/solana/id.json}"
export PROGRAM_ID="${PROGRAM_ID:-9Trpfw7P4YzbaaRQYDS5fmnsAGie5JLQ1FjcgzgJfDq9}"
export EPOCH_ID="${EPOCH_ID:-$(date +%s)}"
export PROVER_BINARY_PATH="${PROVER_BINARY_PATH:-target/release/prover}"
export PROVING_KEY_PATH="${PROVING_KEY_PATH:-artifacts/proving_key.bin}"
export RUST_LOG="${RUST_LOG:-oracle=info}"
set_demo_vrf_secret

VALIDATOR_LEDGER_DIR="${VALIDATOR_LEDGER_DIR:-$ROOT_DIR/target/local-demo-ledger}"
COMMIT_OFFSET_SLOTS="${COMMIT_OFFSET_SLOTS:-200}"
REVEAL_OFFSET_SLOTS="${REVEAL_OFFSET_SLOTS:-450}"
FINALIZE_OFFSET_SLOTS="${FINALIZE_OFFSET_SLOTS:-750}"

step "Checking required commands"
require_cmd cargo
require_cmd solana
require_cmd solana-keygen
require_cmd solana-test-validator
require_cmd anchor

echo "[INFO] Log file: $LOG_FILE"
echo "[INFO] SOLANA_RPC_URL=$SOLANA_RPC_URL"
echo "[INFO] ORACLE_KEYPAIR_PATH=$ORACLE_KEYPAIR_PATH"
echo "[INFO] Initial PROGRAM_ID=$PROGRAM_ID"
echo "[INFO] EPOCH_ID=$EPOCH_ID"
echo "[INFO] RUST_LOG=$RUST_LOG"

ensure_wallet
start_validator_if_needed

ORACLE_PUBKEY="$(solana address --keypair "$ORACLE_KEYPAIR_PATH")"
step "Funding oracle wallet $ORACLE_PUBKEY"
solana airdrop 10 "$ORACLE_PUBKEY" --url "$SOLANA_RPC_URL"

step "Generating local Groth16 proving/verifying artifacts"
backup_source_files
cargo run -p setup -- local-random

sync_anchor_keys_for_local_deploy

step "Building prover binary"
cargo build -p prover --release

step "Building oracle binary"
cargo build -p oracle --bin oracle

step "Building Anchor program"
anchor build --no-idl

step "Deploying Anchor program to local validator"
anchor deploy \
  --provider.cluster "$SOLANA_RPC_URL" \
  --provider.wallet "$ORACLE_KEYPAIR_PATH" \
  2>&1 | tee "$DEPLOY_LOG"

DEPLOYED_PROGRAM_ID="$(awk '/Program Id:/ {print $3}' "$DEPLOY_LOG" | tail -n 1)"
if [[ -z "$DEPLOYED_PROGRAM_ID" ]]; then
  fail "Could not parse deployed program id from $DEPLOY_LOG"
fi
if [[ "$PROGRAM_ID" != "$DEPLOYED_PROGRAM_ID" ]]; then
  echo "[WARN] PROGRAM_ID=$PROGRAM_ID, but anchor deployed $DEPLOYED_PROGRAM_ID"
  echo "[WARN] Using deployed program id for this local demo."
  export PROGRAM_ID="$DEPLOYED_PROGRAM_ID"
fi
echo "[INFO] Active PROGRAM_ID=$PROGRAM_ID"

CURRENT_SLOT="$(solana slot --url "$SOLANA_RPC_URL")"
COMMIT_DEADLINE_SLOT=$((CURRENT_SLOT + COMMIT_OFFSET_SLOTS))
REVEAL_DEADLINE_SLOT=$((CURRENT_SLOT + REVEAL_OFFSET_SLOTS))
FINALIZE_DEADLINE_SLOT=$((CURRENT_SLOT + FINALIZE_OFFSET_SLOTS))

step "Initializing epoch $EPOCH_ID"
echo "[INFO] current_slot=$CURRENT_SLOT"
echo "[INFO] commit_deadline_slot=$COMMIT_DEADLINE_SLOT"
echo "[INFO] reveal_deadline_slot=$REVEAL_DEADLINE_SLOT"
echo "[INFO] finalize_deadline_slot=$FINALIZE_DEADLINE_SLOT"
cargo run -p oracle --bin oracle -- init-epoch \
  --epoch-id "$EPOCH_ID" \
  --commit-deadline-slot "$COMMIT_DEADLINE_SLOT" \
  --reveal-deadline-slot "$REVEAL_DEADLINE_SLOT" \
  --finalize-deadline-slot "$FINALIZE_DEADLINE_SLOT"

step "Running oracle through commit, reveal, and finalize"
cargo run -p oracle --bin oracle -- run

step "Local demo completed"
echo "[OK] Epoch $EPOCH_ID completed successfully."
echo "[OK] Full log: $LOG_FILE"
if [[ "$STARTED_VALIDATOR" == "1" && "${KEEP_VALIDATOR:-0}" == "1" ]]; then
  echo "[OK] Validator is still running with pid $VALIDATOR_PID"
fi

use std::sync::Arc;

use anchor_lang::{AnchorSerialize, Discriminator};
use anyhow::Result;
use async_trait::async_trait;
use solana_program_test::ProgramTest;
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};

use oracle::vrf::VrfOutput;
use randomness_beacon::{EpochPhase, EpochState};

struct BanksRpcAdapter {
    banks: tokio::sync::Mutex<solana_program_test::BanksClient>,
}

#[async_trait]
impl oracle::rpc::RpcProvider for BanksRpcAdapter {
    async fn get_slot(&self) -> Result<u64> {
        let mut banks = self.banks.lock().await;
        let slot = banks.get_root_slot().await?;
        Ok(slot)
    }

    async fn get_account_data(&self, pubkey: &Pubkey) -> Result<Option<Vec<u8>>> {
        let mut banks = self.banks.lock().await;
        let account = banks.get_account(*pubkey).await?;
        Ok(account.map(|a| a.data))
    }

    async fn get_latest_blockhash(&self) -> Result<Hash> {
        let mut banks = self.banks.lock().await;
        let hash = banks.get_latest_blockhash().await?;
        Ok(hash)
    }

    async fn send_and_confirm_transaction(&self, tx: &Transaction) -> Result<Signature> {
        let mut banks = self.banks.lock().await;
        banks.process_transaction(tx.clone()).await?;
        Ok(tx.signatures[0])
    }
}

fn serialize_epoch_state(state: &EpochState) -> Vec<u8> {
    let discriminator = EpochState::discriminator();
    let mut buf = discriminator.to_vec();
    state.serialize(&mut buf).unwrap();
    buf
}

fn epoch_state_pda(program_id: &Pubkey, epoch_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"epoch", &epoch_id.to_le_bytes()], program_id)
}

#[tokio::test]
async fn oracle_reads_initialized_epoch_state() {
    let program_id = randomness_beacon::ID;
    let epoch_state_address = Pubkey::new_unique();

    let expected_state = EpochState {
        epoch_id: 1,
        phase: EpochPhase::Commit,
        commit_deadline_slot: 500,
        reveal_deadline_slot: 1000,
        finalize_deadline_slot: 1500,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let data = serialize_epoch_state(&expected_state);

    let mut pt = ProgramTest::default();
    pt.add_account(
        epoch_state_address,
        Account {
            lamports: 1_000_000,
            data,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks_client, _payer, _recent_blockhash) = pt.start().await;

    let adapter = BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(banks_client),
    };

    let monitor = oracle::epoch::EpochMonitor::new(&adapter, epoch_state_address);

    let state = monitor
        .read_epoch_state()
        .await
        .expect("read_epoch_state failed")
        .expect("account should exist");

    assert_eq!(state.epoch_id, 1);
    assert_eq!(state.commit_deadline_slot, 500);
    assert_eq!(state.reveal_deadline_slot, 1000);
    assert_eq!(state.finalize_deadline_slot, 1500);
    assert_eq!(state.phase, EpochPhase::Commit);
    assert!(!state.is_finalized);

    let phase = monitor
        .current_phase()
        .await
        .expect("current_phase failed")
        .expect("phase should be Some");
    assert_eq!(phase, EpochPhase::Commit);
}

#[tokio::test]
async fn oracle_reads_none_for_missing_epoch() {
    let pt = ProgramTest::default();
    let (banks_client, _payer, _recent_blockhash) = pt.start().await;

    let missing_address = Pubkey::new_unique();

    let adapter = BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(banks_client),
    };

    let monitor = oracle::epoch::EpochMonitor::new(&adapter, missing_address);

    let state = monitor
        .read_epoch_state()
        .await
        .expect("read_epoch_state should not error on missing account");
    assert!(state.is_none());

    let phase = monitor
        .current_phase()
        .await
        .expect("current_phase should not error");
    assert!(phase.is_none());
}

#[tokio::test]
async fn send_commit_stores_commitment_on_chain() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 1;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    // Load the real BPF program from target/deploy/
    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);

    // Pre-populate the epoch PDA with correctly-sized data (matching the current
    // EpochState layout) so the on-chain oracle_commit can read commit_deadline_slot
    // and enforce the deadline check end-to-end.
    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Commit,
        commit_deadline_slot: 10_000,
        reveal_deadline_slot: 20_000,
        finalize_deadline_slot: 30_000,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks_client, payer, _blockhash) = pt.start().await;
    let payer_copy = Keypair::from_bytes(&payer.to_bytes()).unwrap();

    let adapter = BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(banks_client),
    };

    // Submit a commit via the oracle's TxBuilder — exercises the real program's
    // commit_deadline_slot check.
    let salt = [42u8; 32];
    let commitment = oracle::epoch::commitment_hash(&salt);

    let tx_builder = oracle::tx::TxBuilder::new(&adapter, &payer_copy, program_id, epoch_state_pda);

    let sig = tx_builder
        .send_commit(commitment)
        .await
        .expect("send_commit failed");
    assert_ne!(sig, Signature::default());

    // Read back and verify the commitment was stored
    let monitor = oracle::epoch::EpochMonitor::new(&adapter, epoch_state_pda);
    let state = monitor
        .read_epoch_state()
        .await
        .expect("read_epoch_state failed")
        .expect("account should exist");

    assert_eq!(state.commitment, commitment);
    assert_eq!(state.epoch_id, epoch_id);
}

#[tokio::test]
async fn commit_reveal_full_cycle() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 2;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let commit_deadline_slot: u64 = 100;
    let reveal_deadline_slot: u64 = 200;
    let finalize_deadline_slot: u64 = 300;

    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Commit,
        commit_deadline_slot,
        reveal_deadline_slot,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut ctx = pt.start_with_context().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();

    // --- Commit phase: submit commitment ---
    let commit_state = oracle::epoch::CommitState {
        epoch_id,
        salt: [0x42; 32],
    };
    let commitment = commit_state.commitment();

    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(ctx.banks_client.clone()),
    });

    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    let commit_sig = tx_builder
        .send_commit(commitment)
        .await
        .expect("send_commit should succeed in commit phase");
    assert_ne!(commit_sig, Signature::default());

    // --- Advance to reveal phase ---
    ctx.warp_to_slot(commit_deadline_slot + 1).unwrap();
    *adapter.banks.lock().await = ctx.banks_client.clone();

    // --- Reveal phase: submit salt ---
    let reveal_sig = tx_builder
        .send_reveal(&commit_state)
        .await
        .expect("send_reveal should succeed in reveal phase");
    assert_ne!(reveal_sig, Signature::default());

    // --- Verify on-chain state after reveal ---
    let monitor = oracle::epoch::EpochMonitor::new(adapter.as_ref(), epoch_state_pda);
    let state = monitor
        .read_epoch_state()
        .await
        .expect("read_epoch_state failed")
        .expect("account should exist");
    assert_eq!(state.entropy_seed, commit_state.salt);
}

#[tokio::test]
async fn reveal_with_wrong_salt_rejected() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 3;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let commit_deadline_slot: u64 = 100;
    let reveal_deadline_slot: u64 = 200;
    let finalize_deadline_slot: u64 = 300;

    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Commit,
        commit_deadline_slot,
        reveal_deadline_slot,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut ctx = pt.start_with_context().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();

    // Commit with salt A
    let real_salt = [0xAA; 32];
    let commitment = oracle::epoch::commitment_hash(&real_salt);

    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(ctx.banks_client.clone()),
    });
    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    tx_builder
        .send_commit(commitment)
        .await
        .expect("send_commit should succeed");

    // Advance to reveal phase
    ctx.warp_to_slot(commit_deadline_slot + 1).unwrap();
    *adapter.banks.lock().await = ctx.banks_client.clone();

    // Attempt reveal with salt B (wrong salt)
    let wrong_commit_state = oracle::epoch::CommitState {
        epoch_id,
        salt: [0xBB; 32],
    };
    let result = tx_builder.send_reveal(&wrong_commit_state).await;
    assert!(
        result.is_err(),
        "reveal with wrong salt should be rejected (CommitmentMismatch)"
    );
}

#[tokio::test]
async fn reveal_after_deadline_rejected() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 4;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let commit_deadline_slot: u64 = 100;
    let reveal_deadline_slot: u64 = 200;
    let finalize_deadline_slot: u64 = 300;

    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Commit,
        commit_deadline_slot,
        reveal_deadline_slot,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut ctx = pt.start_with_context().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();

    // Commit during commit phase
    let salt = [0xCC; 32];
    let commitment = oracle::epoch::commitment_hash(&salt);

    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(ctx.banks_client.clone()),
    });
    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    tx_builder
        .send_commit(commitment)
        .await
        .expect("send_commit should succeed");

    // Advance PAST the reveal deadline
    ctx.warp_to_slot(reveal_deadline_slot + 1).unwrap();
    *adapter.banks.lock().await = ctx.banks_client.clone();

    // Attempt reveal after deadline
    let commit_state = oracle::epoch::CommitState { epoch_id, salt };
    let result = tx_builder.send_reveal(&commit_state).await;
    assert!(
        result.is_err(),
        "reveal after deadline should be rejected (RevealDeadlinePassed)"
    );
}

#[tokio::test]
async fn finalize_after_deadline_rejected() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 5;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    // Use a very low deadline so the validator's initial slot already exceeds it
    let finalize_deadline_slot: u64 = 1;

    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Finalize,
        commit_deadline_slot: 0,
        reveal_deadline_slot: 0,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut ctx = pt.start_with_context().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();

    // Warp well past the deadline to be certain
    ctx.warp_to_slot(100).unwrap();
    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(ctx.banks_client.clone()),
    });
    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    // Dummy proof — the deadline check fires before the verifier
    let dummy_vrf = VrfOutput {
        output: [0u8; 32],
        proof: vec![0u8; 256],
        public_inputs: vec![[0u8; 32], [0u8; 32]],
    };

    let result = tx_builder.send_finalize(&dummy_vrf).await;
    assert!(
        result.is_err(),
        "finalize after deadline should be rejected (FinalizeDeadlinePassed)"
    );
}

#[tokio::test]
async fn finalize_twice_rejected() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 6;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let finalize_deadline_slot: u64 = 10_000;

    // Pre-populate epoch as already finalized
    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Closed,
        commit_deadline_slot: 100,
        reveal_deadline_slot: 200,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0x11; 32],
        is_finalized: true,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed: [0u8; 32],
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks_client, payer, _blockhash) = pt.start().await;
    let payer = Keypair::from_bytes(&payer.to_bytes()).unwrap();

    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(banks_client),
    });
    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    // Dummy proof — the already-finalized check fires before the verifier
    let dummy_vrf = VrfOutput {
        output: [0u8; 32],
        proof: vec![0u8; 256],
        public_inputs: vec![[0u8; 32], [0u8; 32]],
    };

    let result = tx_builder.send_finalize(&dummy_vrf).await;
    assert!(
        result.is_err(),
        "finalize on already-finalized epoch should be rejected (AlreadyFinalized)"
    );
}

#[tokio::test]
async fn finalize_with_wrong_alpha_hash_rejected() {
    let program_id = randomness_beacon::ID;
    let epoch_id: u64 = 7;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let finalize_deadline_slot: u64 = 10_000;

    // Pre-populate epoch with a known entropy_seed
    let entropy_seed = [0xDD; 32];
    let initial_state = EpochState {
        epoch_id,
        phase: EpochPhase::Finalize,
        commit_deadline_slot: 100,
        reveal_deadline_slot: 200,
        finalize_deadline_slot,
        commitment: [0u8; 32],
        aggregated_seed: [0u8; 32],
        vrf_output: [0u8; 32],
        is_finalized: false,
        entropy_manifest_hash: [0u8; 32],
        entropy_seed,
    };

    let mut pt = ProgramTest::new("randomness_beacon", program_id, None);
    pt.add_account(
        epoch_state_pda,
        Account {
            lamports: 10_000_000,
            data: serialize_epoch_state(&initial_state),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks_client, payer, _blockhash) = pt.start().await;
    let payer = Keypair::from_bytes(&payer.to_bytes()).unwrap();

    let adapter = Arc::new(BanksRpcAdapter {
        banks: tokio::sync::Mutex::new(banks_client),
    });
    let tx_builder =
        oracle::tx::TxBuilder::new(adapter.as_ref(), &payer, program_id, epoch_state_pda);

    // Submit finalize with a wrong alpha_hash (all zeros instead of the correct
    // sha256(entropy_seed) mod r). The deadline and already-finalized guards pass,
    // but the alpha_hash binding check should reject it.
    let wrong_alpha_hash = [0u8; 32];
    let dummy_vrf = VrfOutput {
        output: [0u8; 32],
        proof: vec![0u8; 256],
        public_inputs: vec![wrong_alpha_hash, [0u8; 32]],
    };

    let result = tx_builder.send_finalize(&dummy_vrf).await;
    assert!(
        result.is_err(),
        "finalize with wrong alpha_hash should be rejected (AlphaHashMismatch)"
    );
}

// End-to-end finalize integration test.
//
// This test is #[ignore] because it requires external setup:
//   1. `cargo build -p prover` (produces target/debug/prover binary)
//   2. `cargo run -p setup -- local-random --artifacts artifacts/`
//      (produces artifacts/proving_key.bin)
//
// To run manually:
//   cargo test -p oracle --test epoch_integration finalize_end_to_end -- --ignored
//
// What it would do:
//   - Pre-populate an epoch PDA in Finalize phase with a known entropy_seed
//   - Invoke oracle::finalize::run_finalize with a real prover subprocess
//   - Submit the resulting proof to the BPF program via BanksClient
//   - Assert the epoch transitions to Closed with the expected vrf_output
//
// This exercises the full pipeline: oracle VRF evaluation -> prover subprocess ->
// Groth16 proof generation -> on-chain Groth16 verification. It is the definitive
// test that all components agree on the same cryptographic computation.
#[tokio::test]
#[ignore]
async fn finalize_end_to_end() {
    // See comment above for prerequisites. This test requires:
    // - target/debug/prover binary
    // - artifacts/proving_key.bin
    // Both are produced by `cargo build -p prover` and `cargo run -p setup -- local-random`.
    todo!("implement once prover binary and proving_key.bin are available in CI");
}

use anchor_lang::{AnchorSerialize, Discriminator};
use anyhow::Result;
use async_trait::async_trait;
use solana_program_test::{processor, ProgramTest};
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};

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

/// Minimal processor that mimics the on-chain oracle_commit logic.
/// Accepts instruction data: 8-byte discriminator + 32-byte commitment.
/// Writes the commitment into the epoch_state account at the correct offset.
fn stub_commit_processor(
    _program_id: &Pubkey,
    accounts: &[solana_sdk::account_info::AccountInfo],
    instruction_data: &[u8],
) -> solana_sdk::entrypoint::ProgramResult {
    if instruction_data.len() < 40 {
        return Err(solana_sdk::program_error::ProgramError::InvalidInstructionData);
    }
    let commitment: [u8; 32] = instruction_data[8..40].try_into().unwrap();
    // In the serialized EpochState (after the 8-byte account discriminator):
    //   epoch_id: 8 bytes (offset 0)
    //   phase: 1 byte (offset 8)
    //   commit_deadline_slot: 8 bytes (offset 9)
    //   reveal_deadline_slot: 8 bytes (offset 17)
    //   finalize_deadline_slot: 8 bytes (offset 25)
    //   commitment: 32 bytes (offset 33)
    let epoch_state_account = &accounts[1];
    let mut data = epoch_state_account.try_borrow_mut_data()?;
    // account discriminator (8) + field offset (33) = byte 41
    data[41..73].copy_from_slice(&commitment);
    Ok(())
}

#[tokio::test]
async fn send_commit_stores_commitment_on_chain() {
    // randomness_beacon::ID is currently a placeholder (system program); use a unique ID for test
    let program_id = Pubkey::new_unique();
    let epoch_id: u64 = 1;
    let (epoch_state_pda, _bump) = epoch_state_pda(&program_id, epoch_id);

    let mut pt = ProgramTest::new(
        "randomness_beacon",
        program_id,
        processor!(stub_commit_processor),
    );
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

    let salt = [42u8; 32];
    let commitment = oracle::epoch::commitment_hash(&salt);

    let tx_builder = oracle::tx::TxBuilder::new(
        &adapter,
        &payer_copy,
        program_id,
        epoch_state_pda,
    );

    let sig = tx_builder.send_commit(commitment).await.expect("send_commit failed");
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

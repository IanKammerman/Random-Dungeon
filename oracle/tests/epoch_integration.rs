use anchor_lang::{AnchorSerialize, Discriminator};
use anyhow::Result;
use async_trait::async_trait;
use solana_program_test::ProgramTest;
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
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
}

fn serialize_epoch_state(state: &EpochState) -> Vec<u8> {
    let discriminator = EpochState::discriminator();
    let mut buf = discriminator.to_vec();
    state.serialize(&mut buf).unwrap();
    buf
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

    let monitor = oracle::epoch::EpochMonitor::new(adapter, epoch_state_address);

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

    // Current slot starts near 0, which is < commit_deadline 500 → Commit phase
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

    let monitor = oracle::epoch::EpochMonitor::new(adapter, missing_address);

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

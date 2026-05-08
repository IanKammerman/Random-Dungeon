use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use randomness_beacon::EpochPhase;
use randomness_beacon::EpochState;

use crate::rpc::RpcProvider;

pub struct EpochMonitor<R: RpcProvider> {
    pub rpc: R,
    pub epoch_state_address: Pubkey,
}

impl<R: RpcProvider> EpochMonitor<R> {
    pub fn new(rpc: R, epoch_state_address: Pubkey) -> Self {
        Self {
            rpc,
            epoch_state_address,
        }
    }

    pub async fn read_epoch_state(&self) -> Result<Option<EpochState>> {
        let data = self.rpc.get_account_data(&self.epoch_state_address).await?;
        match data {
            None => Ok(None),
            Some(raw) => {
                use anchor_lang::AnchorDeserialize;
                // Skip the 8-byte Anchor discriminator
                let state = EpochState::deserialize(&mut &raw[8..])?;
                Ok(Some(state))
            }
        }
    }

    pub async fn current_phase(&self) -> Result<Option<EpochPhase>> {
        let state = match self.read_epoch_state().await? {
            Some(s) => s,
            None => return Ok(None),
        };
        let current_slot = self.rpc.get_slot().await?;
        Ok(Some(derive_phase(&state, current_slot)))
    }

    pub async fn wait_for_phase(&self, _target: EpochPhase) -> Result<EpochState> {
        // TODO: poll until the on-chain phase matches target
        self.read_epoch_state()
            .await?
            .ok_or_else(|| anyhow::anyhow!("epoch state account does not exist"))
    }
}

pub fn derive_phase(state: &EpochState, current_slot: u64) -> EpochPhase {
    if state.is_finalized {
        return EpochPhase::Closed;
    }
    if current_slot <= state.commit_deadline_slot {
        EpochPhase::Commit
    } else if current_slot <= state.reveal_deadline_slot {
        EpochPhase::Reveal
    } else if current_slot <= state.finalize_deadline_slot {
        EpochPhase::Finalize
    } else {
        EpochPhase::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockRpc {
        slot: u64,
        account_data: Mutex<Option<Vec<u8>>>,
    }

    impl MockRpc {
        fn with_state(slot: u64, state: Option<&EpochState>) -> Self {
            let account_data = state.map(|s| {
                use anchor_lang::{AnchorSerialize, Discriminator};
                let discriminator = EpochState::discriminator();
                let mut buf = discriminator.to_vec();
                s.serialize(&mut buf).unwrap();
                buf
            });
            Self {
                slot,
                account_data: Mutex::new(account_data),
            }
        }
    }

    #[async_trait]
    impl RpcProvider for MockRpc {
        async fn get_slot(&self) -> Result<u64> {
            Ok(self.slot)
        }

        async fn get_account_data(&self, _pubkey: &Pubkey) -> Result<Option<Vec<u8>>> {
            Ok(self.account_data.lock().unwrap().clone())
        }
    }

    fn sample_epoch_state() -> EpochState {
        EpochState {
            epoch_id: 1,
            phase: EpochPhase::Commit, // stored phase doesn't matter; we derive from slots
            commit_deadline_slot: 100,
            reveal_deadline_slot: 200,
            finalize_deadline_slot: 300,
            commitment: [0u8; 32],
            aggregated_seed: [0u8; 32],
            vrf_output: [0u8; 32],
            is_finalized: false,
            entropy_manifest_hash: [0u8; 32],
            entropy_seed: [0u8; 32],
        }
    }

    #[tokio::test]
    async fn read_epoch_state_returns_none_when_account_missing() {
        let rpc = MockRpc::with_state(50, None);
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let result = monitor.read_epoch_state().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn current_phase_returns_none_when_account_missing() {
        let rpc = MockRpc::with_state(50, None);
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert!(phase.is_none());
    }

    #[tokio::test]
    async fn current_phase_commit_when_slot_before_commit_deadline() {
        let state = sample_epoch_state();
        let rpc = MockRpc::with_state(50, Some(&state));
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert_eq!(phase, Some(EpochPhase::Commit));
    }

    #[tokio::test]
    async fn current_phase_reveal_when_slot_past_commit_deadline() {
        let state = sample_epoch_state();
        let rpc = MockRpc::with_state(150, Some(&state));
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert_eq!(phase, Some(EpochPhase::Reveal));
    }

    #[tokio::test]
    async fn current_phase_finalize_when_slot_past_reveal_deadline() {
        let state = sample_epoch_state();
        let rpc = MockRpc::with_state(250, Some(&state));
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert_eq!(phase, Some(EpochPhase::Finalize));
    }

    #[tokio::test]
    async fn current_phase_closed_when_slot_past_finalize_deadline() {
        let state = sample_epoch_state();
        let rpc = MockRpc::with_state(350, Some(&state));
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert_eq!(phase, Some(EpochPhase::Closed));
    }

    #[tokio::test]
    async fn current_phase_closed_when_finalized() {
        let mut state = sample_epoch_state();
        state.is_finalized = true;
        // Even though slot is in "commit" range, finalized flag wins
        let rpc = MockRpc::with_state(50, Some(&state));
        let monitor = EpochMonitor::new(rpc, Pubkey::new_unique());
        let phase = monitor.current_phase().await.unwrap();
        assert_eq!(phase, Some(EpochPhase::Closed));
    }

    #[test]
    fn derive_phase_boundary_conditions() {
        let state = sample_epoch_state();
        // Exactly at commit deadline → still Commit
        assert_eq!(derive_phase(&state, 100), EpochPhase::Commit);
        // One past commit deadline → Reveal
        assert_eq!(derive_phase(&state, 101), EpochPhase::Reveal);
        // Exactly at reveal deadline → still Reveal
        assert_eq!(derive_phase(&state, 200), EpochPhase::Reveal);
        // One past → Finalize
        assert_eq!(derive_phase(&state, 201), EpochPhase::Finalize);
        // Exactly at finalize deadline → still Finalize
        assert_eq!(derive_phase(&state, 300), EpochPhase::Finalize);
        // One past → Closed
        assert_eq!(derive_phase(&state, 301), EpochPhase::Closed);
    }
}

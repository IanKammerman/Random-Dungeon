use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use randomness_beacon::EpochPhase;
use randomness_beacon::EpochState;

use crate::rpc::SolanaRpc;

pub struct EpochMonitor<'a> {
    pub rpc: &'a SolanaRpc,
    pub epoch_state_address: Pubkey,
}

impl<'a> EpochMonitor<'a> {
    pub fn new(rpc: &'a SolanaRpc, epoch_state_address: Pubkey) -> Self {
        Self {
            rpc,
            epoch_state_address,
        }
    }

    pub async fn current_phase(&self) -> Result<EpochPhase> {
        let state = self.read_epoch_state().await?;
        Ok(state.phase)
    }

    pub async fn read_epoch_state(&self) -> Result<EpochState> {
        let data = self.rpc.get_account_data(&self.epoch_state_address).await?;
        // Skip the 8-byte Anchor discriminator
        let state = EpochState::try_from_slice(&data[8..])?;
        Ok(state)
    }

    pub async fn wait_for_phase(&self, _target: EpochPhase) -> Result<EpochState> {
        // TODO: poll until the on-chain phase matches target
        self.read_epoch_state().await
    }
}

trait EpochStateDeserialize {
    fn try_from_slice(data: &[u8]) -> Result<EpochState>;
}

impl EpochStateDeserialize for EpochState {
    fn try_from_slice(data: &[u8]) -> Result<EpochState> {
        use anchor_lang::AnchorDeserialize;
        let state = EpochState::deserialize(&mut &data[..])?;
        Ok(state)
    }
}

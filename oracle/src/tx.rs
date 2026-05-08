use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;

use crate::rpc::SolanaRpc;
use crate::vrf::VrfOutput;

pub struct TxBuilder<'a> {
    pub rpc: &'a SolanaRpc,
    pub epoch_state_address: Pubkey,
}

impl<'a> TxBuilder<'a> {
    pub fn new(rpc: &'a SolanaRpc, epoch_state_address: Pubkey) -> Self {
        Self {
            rpc,
            epoch_state_address,
        }
    }

    pub async fn send_commit(&self, _commitment: [u8; 32]) -> Result<Signature> {
        // TODO: build and send oracle_commit instruction
        todo!()
    }

    pub async fn send_reveal(&self, _seed: [u8; 32]) -> Result<Signature> {
        // TODO: build and send oracle_reveal instruction
        todo!()
    }

    pub async fn send_finalize(&self, _vrf_output: &VrfOutput) -> Result<Signature> {
        // TODO: build and send finalize_epoch instruction
        todo!()
    }

    pub async fn read_epoch_state(&self) -> Result<randomness_beacon::EpochState> {
        use crate::rpc::RpcProvider;
        use anchor_lang::AnchorDeserialize;
        let data = self.rpc.get_account_data(&self.epoch_state_address).await?
            .ok_or_else(|| anyhow::anyhow!("epoch state account not found"))?;
        let state = randomness_beacon::EpochState::deserialize(&mut &data[8..])?;
        Ok(state)
    }
}

use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use crate::config::Config;

pub struct SolanaRpc {
    pub client: RpcClient,
    pub payer: Keypair,
    pub program_id: Pubkey,
}

impl SolanaRpc {
    pub fn new(cfg: &Config, payer: Keypair) -> Self {
        let client = RpcClient::new_with_commitment(
            cfg.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );
        Self {
            client,
            payer,
            program_id: cfg.program_id,
        }
    }

    pub async fn get_slot(&self) -> Result<u64> {
        let slot = self.client.get_slot().await?;
        Ok(slot)
    }

    pub async fn get_account_data(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {
        let account = self.client.get_account(pubkey).await?;
        Ok(account.data)
    }
}

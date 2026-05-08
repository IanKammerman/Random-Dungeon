use anyhow::Result;
use async_trait::async_trait;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use crate::config::Config;

#[async_trait]
pub trait RpcProvider: Send + Sync {
    async fn get_slot(&self) -> Result<u64>;
    async fn get_account_data(&self, pubkey: &Pubkey) -> Result<Option<Vec<u8>>>;
}

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
}

#[async_trait]
impl RpcProvider for SolanaRpc {
    async fn get_slot(&self) -> Result<u64> {
        let slot = self.client.get_slot().await?;
        Ok(slot)
    }

    async fn get_account_data(&self, pubkey: &Pubkey) -> Result<Option<Vec<u8>>> {
        use solana_client::client_error::ClientErrorKind;
        use solana_client::rpc_request::RpcError;

        match self.client.get_account(pubkey).await {
            Ok(account) => Ok(Some(account.data)),
            Err(e) => {
                if let ClientErrorKind::RpcError(RpcError::ForUser(ref msg)) = *e.kind() {
                    if msg.contains("AccountNotFound") || msg.contains("could not find account") {
                        return Ok(None);
                    }
                }
                Err(e.into())
            }
        }
    }
}

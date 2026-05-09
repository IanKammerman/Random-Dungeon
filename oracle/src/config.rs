use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use solana_sdk::pubkey::Pubkey;

use crate::vrf::OracleVrf;

pub struct Config {
    pub rpc_url: String,
    pub keypair_path: PathBuf,
    pub program_id: Pubkey,
    pub vrf: OracleVrf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let rpc_url = env::var("SOLANA_RPC_URL")
            .context("SOLANA_RPC_URL not set")?;

        let keypair_path = env::var("ORACLE_KEYPAIR_PATH")
            .map(PathBuf::from)
            .context("ORACLE_KEYPAIR_PATH not set")?;

        let program_id = env::var("PROGRAM_ID")
            .context("PROGRAM_ID not set")?
            .parse::<Pubkey>()
            .context("PROGRAM_ID is not a valid pubkey")?;

        let vrf = OracleVrf::from_env()?;

        Ok(Self {
            rpc_url,
            keypair_path,
            program_id,
            vrf,
        })
    }
}

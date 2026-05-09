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
    pub vrf_secret_hex: String,
    pub prover_binary_path: PathBuf,
    pub proving_key_path: PathBuf,
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

        let vrf_secret_hex = env::var("ORACLE_VRF_SECRET")
            .context("ORACLE_VRF_SECRET not set")?;

        let vrf = OracleVrf::from_env()?;

        let prover_binary_path = env::var("PROVER_BINARY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_prover_path());

        let proving_key_path = env::var("PROVING_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("artifacts/proving_key.bin"));

        Ok(Self {
            rpc_url,
            keypair_path,
            program_id,
            vrf,
            vrf_secret_hex,
            prover_binary_path,
            proving_key_path,
        })
    }
}

fn default_prover_path() -> PathBuf {
    let release = PathBuf::from("target/release/prover");
    if release.exists() {
        return release;
    }
    PathBuf::from("target/debug/prover")
}

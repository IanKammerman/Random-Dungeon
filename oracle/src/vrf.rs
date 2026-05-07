use std::path::Path;

use anyhow::{Context, Result};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

pub struct VrfKeyPair {
    keypair: Keypair,
}

pub struct VrfOutput {
    pub output: [u8; 32],
    pub proof: Vec<u8>,
}

impl VrfKeyPair {
    pub fn from_file(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .context("failed to read VRF keypair file")?;
        let keypair = Keypair::from_bytes(&bytes)
            .context("invalid keypair bytes")?;
        Ok(Self { keypair })
    }

    pub fn public_key(&self) -> solana_sdk::pubkey::Pubkey {
        self.keypair.pubkey()
    }

    pub fn evaluate(&self, _input: &[u8; 32]) -> Result<VrfOutput> {
        // TODO: implement real ECVRF or signature-as-VUF — TBD
        // For now this is a stub that will be replaced once we decide
        // on the VRF construction.
        todo!()
    }
}

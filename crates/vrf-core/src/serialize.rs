use anyhow::{anyhow, Context, Result};
use ark_bn254::Fr;
use ark_ff::PrimeField;
use serde::{Deserialize, Serialize};

use crate::hash::fr_to_be_bytes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicInputs {
    pub alpha_hash: String,
    pub beta: String,
}

impl PublicInputs {
    pub fn new(alpha_hash: Fr, beta: Fr) -> Self {
        Self {
            alpha_hash: fr_to_hex(&alpha_hash),
            beta: fr_to_hex(&beta),
        }
    }

    pub fn to_field_elements(&self) -> Result<Vec<Fr>> {
        Ok(vec![fr_from_hex(&self.alpha_hash)?, fr_from_hex(&self.beta)?])
    }

    pub fn to_solana_bytes(&self) -> Result<[[u8; 32]; 2]> {
        let inputs = self.to_field_elements()?;
        Ok([fr_to_be_bytes(&inputs[0]), fr_to_be_bytes(&inputs[1])])
    }

    pub fn to_solana_flat_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_solana_bytes()?.into_iter().flatten().collect())
    }
}

pub fn fr_to_hex(value: &Fr) -> String {
    format!("0x{}", hex::encode(fr_to_be_bytes(value)))
}

pub fn fr_from_hex(value: &str) -> Result<Fr> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(value).context("invalid hex field element")?;
    if bytes.len() > 32 {
        return Err(anyhow!("field element is longer than 32 bytes"));
    }
    Ok(Fr::from_be_bytes_mod_order(&bytes))
}

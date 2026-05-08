use anchor_lang::prelude::*;
use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};

use crate::VrfProofError;

const PUBLIC_INPUT_COUNT: usize = 2;
const VK_IC: [[u8; 64]; PUBLIC_INPUT_COUNT + 1] = [[0u8; 64]; PUBLIC_INPUT_COUNT + 1];

// Placeholder key for compilation and integration wiring. Replace this with the
// generated constants from `artifacts/verifying_key_solana.rs` before deploying.
pub const VERIFYING_KEY: Groth16Verifyingkey = Groth16Verifyingkey {
    nr_pubinputs: PUBLIC_INPUT_COUNT,
    vk_alpha_g1: [0u8; 64],
    vk_beta_g2: [0u8; 128],
    vk_gamme_g2: [0u8; 128],
    vk_delta_g2: [0u8; 128],
    vk_ic: &VK_IC,
};

pub fn verify_vrf_proof(proof: &[u8], public_inputs: &[[u8; 32]]) -> Result<[u8; 32]> {
    require!(proof.len() == 256, VrfProofError::InvalidProofLength);
    require!(
        public_inputs.len() == PUBLIC_INPUT_COUNT,
        VrfProofError::InvalidPublicInputCount
    );

    let proof_a: [u8; 64] = proof[0..64]
        .try_into()
        .map_err(|_| error!(VrfProofError::InvalidProofLength))?;
    let proof_b: [u8; 128] = proof[64..192]
        .try_into()
        .map_err(|_| error!(VrfProofError::InvalidProofLength))?;
    let proof_c: [u8; 64] = proof[192..256]
        .try_into()
        .map_err(|_| error!(VrfProofError::InvalidProofLength))?;
    let public_inputs: [[u8; 32]; PUBLIC_INPUT_COUNT] = public_inputs
        .try_into()
        .map_err(|_| error!(VrfProofError::InvalidPublicInputCount))?;

    let mut verifier = Groth16Verifier::new(
        &proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
        &VERIFYING_KEY,
    )
    .map_err(|_| error!(VrfProofError::Groth16VerificationFailed))?;

    verifier
        .verify()
        .map_err(|_| error!(VrfProofError::Groth16VerificationFailed))?;

    Ok(public_inputs[1])
}


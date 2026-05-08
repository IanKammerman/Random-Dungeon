use anchor_lang::prelude::*;
use groth16_solana::groth16::Groth16Verifier;

use crate::VrfProofError;

#[allow(dead_code)]
mod generated_vk {
    include!("../../../artifacts/verifying_key_solana.rs");
}

pub fn verify_vrf_proof(proof: &[u8], public_inputs: &[[u8; 32]]) -> Result<[u8; 32]> {
    require!(proof.len() == 256, VrfProofError::InvalidProofLength);
    require!(
        public_inputs.len() == generated_vk::PUBLIC_INPUT_COUNT,
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
    let public_inputs: [[u8; 32]; generated_vk::PUBLIC_INPUT_COUNT] = public_inputs
        .try_into()
        .map_err(|_| error!(VrfProofError::InvalidPublicInputCount))?;

    let mut verifier = Groth16Verifier::new(
        &proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
        &generated_vk::VERIFYING_KEY,
    )
    .map_err(|_| error!(VrfProofError::Groth16VerificationFailed))?;

    verifier
        .verify()
        .map_err(|_| error!(VrfProofError::Groth16VerificationFailed))?;

    Ok(public_inputs[1])
}

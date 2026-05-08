use anchor_lang::prelude::*;

pub mod state;
pub mod verifier;

use state::VrfProofRecord;

declare_id!("Fg6PaFpoGXkYsidMpWxTWqnbW8VY2GzZ4HLJYibweX4");

#[program]
pub mod ecvrf_solana_program {
    use super::*;

    pub fn verify_vrf_proof(
        ctx: Context<VerifyVrfProof>,
        proof: Vec<u8>,
        public_inputs: Vec<[u8; 32]>,
    ) -> Result<()> {
        let beta = verifier::verify_vrf_proof(&proof, &public_inputs)?;
        let record = &mut ctx.accounts.record;
        record.authority = ctx.accounts.authority.key();
        record.beta = beta;
        record.public_inputs = public_inputs;
        record.accepted = true;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyVrfProof<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + VrfProofRecord::INIT_SPACE,
        seeds = [b"vrf-proof-record", authority.key().as_ref()],
        bump
    )]
    pub record: Account<'info, VrfProofRecord>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum VrfProofError {
    #[msg("expected a 256 byte Groth16 proof encoded as A || B || C")]
    InvalidProofLength,
    #[msg("expected exactly two public inputs: [alpha_hash, beta]")]
    InvalidPublicInputCount,
    #[msg("Groth16 proof verification failed")]
    Groth16VerificationFailed,
}

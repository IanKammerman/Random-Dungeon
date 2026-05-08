use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct VrfProofRecord {
    pub authority: Pubkey,
    pub beta: [u8; 32],
    #[max_len(2)]
    pub public_inputs: Vec<[u8; 32]>,
    pub accepted: bool,
}


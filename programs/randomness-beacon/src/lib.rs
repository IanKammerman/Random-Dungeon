use anchor_lang::prelude::*;

pub mod verifier;

declare_id!("9Trpfw7P4YzbaaRQYDS5fmnsAGie5JLQ1FjcgzgJfDq9");

#[program]
pub mod randomness_beacon {
    use super::*;

    pub fn initialize_epoch(
        ctx: Context<InitializeEpoch>,
        epoch_id: u64,
        commit_deadline_slot: u64,
        reveal_deadline_slot: u64,
        finalize_deadline_slot: u64,
    ) -> Result<()> {
        let state = &mut ctx.accounts.epoch_state;
        state.epoch_id = epoch_id;
        state.phase = EpochPhase::Commit;
        state.commit_deadline_slot = commit_deadline_slot;
        state.reveal_deadline_slot = reveal_deadline_slot;
        state.finalize_deadline_slot = finalize_deadline_slot;
        state.commitment = [0u8; 32];
        state.aggregated_seed = [0u8; 32];
        state.vrf_output = [0u8; 32];
        state.is_finalized = false;
        Ok(())
    }

    pub fn oracle_commit(ctx: Context<OracleCommit>, commitment: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let state = &mut ctx.accounts.epoch_state;
        require!(
            clock.slot <= state.commit_deadline_slot,
            BeaconError::CommitDeadlinePassed
        );
        state.commitment = commitment;
        Ok(())
    }

    pub fn oracle_reveal(ctx: Context<OracleReveal>, seed: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let state = &mut ctx.accounts.epoch_state;
        require!(
            clock.slot <= state.reveal_deadline_slot,
            BeaconError::RevealDeadlinePassed
        );
        require!(
            state.commitment != [0u8; 32],
            BeaconError::CommitmentNotSet
        );
        require!(
            state.entropy_seed == [0u8; 32],
            BeaconError::AlreadyRevealed
        );
        let hash = anchor_lang::solana_program::hash::hash(&seed);
        require!(
            hash.to_bytes() == state.commitment,
            BeaconError::CommitmentMismatch
        );
        state.entropy_seed = seed;
        Ok(())
    }

    pub fn finalize_epoch(
        ctx: Context<FinalizeEpoch>,
        vrf_output: [u8; 32],
        proof: Vec<u8>,
        public_inputs: Vec<[u8; 32]>,
    ) -> Result<()> {
        let state = &mut ctx.accounts.epoch_state;
        let verified_output = verifier::verify_vrf_proof(&proof, &public_inputs)?;
        require!(
            verified_output == vrf_output,
            BeaconError::VrfOutputMismatch
        );

        state.vrf_output = vrf_output;
        state.is_finalized = true;
        state.phase = EpochPhase::Closed;
        Ok(())
    }
}

// --- Account contexts ---

#[derive(Accounts)]
#[instruction(epoch_id: u64)]
pub struct InitializeEpoch<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<EpochState>(),
        seeds = [b"epoch", epoch_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub epoch_state: Account<'info, EpochState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OracleCommit<'info> {
    #[account(mut)]
    pub oracle: Signer<'info>,
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,
}

#[derive(Accounts)]
pub struct OracleReveal<'info> {
    #[account(mut)]
    pub oracle: Signer<'info>,
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,
}

#[derive(Accounts)]
pub struct FinalizeEpoch<'info> {
    #[account(mut)]
    pub oracle: Signer<'info>,
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,
}

// --- Errors ---

#[error_code]
pub enum BeaconError {
    #[msg("Commit deadline has passed")]
    CommitDeadlinePassed,
    #[msg("Reveal deadline has passed")]
    RevealDeadlinePassed,
    #[msg("No commitment has been set for this epoch")]
    CommitmentNotSet,
    #[msg("Oracle has already revealed for this epoch")]
    AlreadyRevealed,
    #[msg("SHA-256(seed) does not match stored commitment")]
    CommitmentMismatch,
    #[msg("expected a 256 byte Groth16 proof encoded as A || B || C")]
    InvalidProofLength,
    #[msg("expected exactly two public inputs: [alpha_hash, beta]")]
    InvalidPublicInputCount,
    #[msg("Groth16 proof verification failed")]
    Groth16VerificationFailed,
    #[msg("verified VRF output does not match the submitted output")]
    VrfOutputMismatch,
}

// --- On-chain state ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum EpochPhase {
    Commit,
    Reveal,
    Finalize,
    Closed,
}

#[account]
#[derive(Debug)]
pub struct EpochState {
    pub epoch_id: u64,
    pub phase: EpochPhase,
    pub commit_deadline_slot: u64,
    pub reveal_deadline_slot: u64,
    pub finalize_deadline_slot: u64,
    pub commitment: [u8; 32],
    pub aggregated_seed: [u8; 32],
    pub vrf_output: [u8; 32],
    pub is_finalized: bool,
    // --- new fields for the entropy module ---
    pub entropy_manifest_hash: [u8; 32],
    pub entropy_seed: [u8; 32],
}

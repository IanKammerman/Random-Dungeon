use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

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

    pub fn oracle_commit(_ctx: Context<OracleCommit>, _commitment: [u8; 32]) -> Result<()> {
        Ok(())
    }

    pub fn oracle_reveal(_ctx: Context<OracleReveal>, _seed: [u8; 32]) -> Result<()> {
        Ok(())
    }

    pub fn finalize_epoch(
        _ctx: Context<FinalizeEpoch>,
        _vrf_output: [u8; 32],
        _proof: Vec<u8>,
    ) -> Result<()> {
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

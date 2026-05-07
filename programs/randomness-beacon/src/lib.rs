use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod randomness_beacon {
    use super::*;

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
}

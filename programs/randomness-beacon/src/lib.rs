use anchor_lang::prelude::*;

pub mod verifier;

declare_id!("2sUazVqcMp31TW5iGKKvEoKM5J8oZGNGf29YDahp2WHH");

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
        state.oracle_pubkey = ctx.accounts.authority.key();
        state.commitment = [0u8; 32];
        state.aggregated_seed = [0u8; 32];
        state.vrf_output = [0u8; 32];
        state.is_finalized = false;
        state.entropy_manifest_hash = [0u8; 32];
        state.entropy_seed = [0u8; 32];
        Ok(())
    }

    pub fn oracle_commit(ctx: Context<OracleCommit>, commitment: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let state = &mut ctx.accounts.epoch_state;
        require_oracle(&ctx.accounts.oracle, state)?;
        require!(
            clock.slot <= state.commit_deadline_slot,
            BeaconError::CommitDeadlinePassed
        );
        state.commitment = commitment;
        state.phase = EpochPhase::Commit;
        Ok(())
    }

    pub fn oracle_reveal(
        ctx: Context<OracleReveal>,
        salt: [u8; 32],
        manifest_hash: [u8; 32],
    ) -> Result<()> {
        let clock = Clock::get()?;
        let state = &mut ctx.accounts.epoch_state;
        require_oracle(&ctx.accounts.oracle, state)?;
        require!(state.commitment != [0u8; 32], BeaconError::CommitmentNotSet);
        require!(
            state.entropy_seed == [0u8; 32],
            BeaconError::AlreadyRevealed
        );
        require!(
            clock.slot > state.commit_deadline_slot,
            BeaconError::RevealBeforeCommitDeadline
        );
        require!(
            clock.slot <= state.reveal_deadline_slot,
            BeaconError::RevealDeadlinePassed
        );
        require!(
            commitment_hash(&salt) == state.commitment,
            BeaconError::CommitmentMismatch
        );

        let seed = oracle_seed(&salt, &manifest_hash);
        state.entropy_manifest_hash = manifest_hash;
        state.entropy_seed = seed;
        state.aggregated_seed = seed;
        state.phase = EpochPhase::Reveal;
        Ok(())
    }

    pub fn finalize_epoch(
        ctx: Context<FinalizeEpoch>,
        vrf_output: [u8; 32],
        proof: Vec<u8>,
        public_inputs: Vec<[u8; 32]>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let state = &mut ctx.accounts.epoch_state;
        require_oracle(&ctx.accounts.oracle, state)?;
        require!(
            clock.slot > state.reveal_deadline_slot,
            BeaconError::FinalizeBeforeRevealDeadline
        );
        require!(
            clock.slot <= state.finalize_deadline_slot,
            BeaconError::FinalizeDeadlinePassed
        );
        require!(!state.is_finalized, BeaconError::AlreadyFinalized);

        let entropy_hash = anchor_lang::solana_program::hash::hash(&state.entropy_seed);
        let computed_alpha_hash = reduce_be_bytes_mod_r(&entropy_hash.to_bytes());
        require!(
            public_inputs.first() == Some(&computed_alpha_hash),
            BeaconError::AlphaHashMismatch
        );

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

fn require_oracle(oracle: &Signer<'_>, state: &EpochState) -> Result<()> {
    require!(
        oracle.key() == state.oracle_pubkey,
        BeaconError::UnauthorizedOracle
    );
    Ok(())
}

fn commitment_hash(salt: &[u8; 32]) -> [u8; 32] {
    anchor_lang::solana_program::hash::hashv(&[salt]).to_bytes()
}

fn oracle_seed(salt: &[u8; 32], manifest_hash: &[u8; 32]) -> [u8; 32] {
    anchor_lang::solana_program::hash::hashv(&[
        b"random-dungeon/oracle-seed/v1",
        salt,
        manifest_hash,
    ])
    .to_bytes()
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
    #[msg("signer is not the oracle registered for this epoch")]
    UnauthorizedOracle,
    #[msg("Commit deadline has passed")]
    CommitDeadlinePassed,
    #[msg("Reveal attempted before the commit deadline")]
    RevealBeforeCommitDeadline,
    #[msg("Reveal deadline has passed")]
    RevealDeadlinePassed,
    #[msg("No commitment has been set for this epoch")]
    CommitmentNotSet,
    #[msg("Oracle has already revealed for this epoch")]
    AlreadyRevealed,
    #[msg("Oracle reveal does not match the committed salt")]
    CommitmentMismatch,
    #[msg("Finalize attempted before the reveal deadline")]
    FinalizeBeforeRevealDeadline,
    #[msg("Finalize deadline has passed")]
    FinalizeDeadlinePassed,
    #[msg("expected a 256 byte Groth16 proof encoded as A || B || C")]
    InvalidProofLength,
    #[msg("expected exactly two public inputs: [alpha_hash, beta]")]
    InvalidPublicInputCount,
    #[msg("Groth16 proof verification failed")]
    Groth16VerificationFailed,
    #[msg("verified VRF output does not match the submitted output")]
    VrfOutputMismatch,
    #[msg("Epoch has already been finalized")]
    AlreadyFinalized,
    #[msg("public_inputs[0] does not match sha256(entropy_seed) reduced mod BN254 r")]
    AlphaHashMismatch,
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
    pub oracle_pubkey: Pubkey,
    pub commitment: [u8; 32],
    pub aggregated_seed: [u8; 32],
    pub vrf_output: [u8; 32],
    pub is_finalized: bool,
    // --- new fields for the entropy module ---
    pub entropy_manifest_hash: [u8; 32],
    pub entropy_seed: [u8; 32],
}

// BN254 scalar field modulus r as four u64 big-endian limbs (most significant first).
const R: [u64; 4] = [
    0x30644E72E131A029,
    0xB85045B68181585D,
    0x2833E84879B97091,
    0x43E1F593F0000001,
];

/// Reduce a 32-byte big-endian integer modulo the BN254 scalar field order r.
/// Returns the canonical 32-byte big-endian representation.
pub fn reduce_be_bytes_mod_r(bytes: &[u8; 32]) -> [u8; 32] {
    let mut v = be_bytes_to_limbs(bytes);
    while gte_r(&v) {
        sub_r(&mut v);
    }
    limbs_to_be_bytes(&v)
}

fn be_bytes_to_limbs(bytes: &[u8; 32]) -> [u64; 4] {
    [
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
        u64::from_be_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]),
        u64::from_be_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]),
    ]
}

fn limbs_to_be_bytes(v: &[u64; 4]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&v[0].to_be_bytes());
    out[8..16].copy_from_slice(&v[1].to_be_bytes());
    out[16..24].copy_from_slice(&v[2].to_be_bytes());
    out[24..32].copy_from_slice(&v[3].to_be_bytes());
    out
}

/// Returns true if v >= R (big-endian limb comparison).
fn gte_r(v: &[u64; 4]) -> bool {
    for i in 0..4 {
        if v[i] > R[i] {
            return true;
        }
        if v[i] < R[i] {
            return false;
        }
    }
    true // v == R
}

/// Subtract R from v in place. Assumes v >= R.
fn sub_r(v: &mut [u64; 4]) {
    let mut borrow: u64 = 0;
    // Process from least significant limb (index 3) to most significant (index 0).
    for i in (0..4).rev() {
        let (diff, b1) = v[i].overflowing_sub(R[i]);
        let (diff2, b2) = diff.overflowing_sub(borrow);
        v[i] = diff2;
        borrow = (b1 as u64) + (b2 as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_ff::{BigInteger, PrimeField};

    fn ark_reduce(bytes: &[u8; 32]) -> [u8; 32] {
        let fr = Fr::from_be_bytes_mod_order(bytes);
        let le_bytes = fr.into_bigint().to_bytes_le();
        let mut out = [0u8; 32];
        for (i, &b) in le_bytes.iter().enumerate() {
            out[31 - i] = b;
        }
        out
    }

    #[test]
    fn reduce_zero() {
        let input = [0u8; 32];
        assert_eq!(reduce_be_bytes_mod_r(&input), ark_reduce(&input));
        assert_eq!(reduce_be_bytes_mod_r(&input), [0u8; 32]);
    }

    #[test]
    fn reduce_modulus_itself_yields_zero() {
        let r_bytes = limbs_to_be_bytes(&R);
        assert_eq!(reduce_be_bytes_mod_r(&r_bytes), [0u8; 32]);
        assert_eq!(reduce_be_bytes_mod_r(&r_bytes), ark_reduce(&r_bytes));
    }

    #[test]
    fn reduce_value_greater_than_r() {
        // r + 1
        let mut r_plus_one = R;
        r_plus_one[3] = r_plus_one[3].wrapping_add(1);
        let input = limbs_to_be_bytes(&r_plus_one);
        let expected = ark_reduce(&input);
        assert_eq!(reduce_be_bytes_mod_r(&input), expected);

        // Should be [0, 0, ..., 0, 1]
        let mut one = [0u8; 32];
        one[31] = 1;
        assert_eq!(expected, one);
    }

    #[test]
    fn reduce_max_u256() {
        let input = [0xFFu8; 32];
        assert_eq!(reduce_be_bytes_mod_r(&input), ark_reduce(&input));
    }

    #[test]
    fn reduce_value_less_than_r_unchanged() {
        // A value clearly below r
        let mut input = [0u8; 32];
        input[31] = 42;
        assert_eq!(reduce_be_bytes_mod_r(&input), input);
        assert_eq!(reduce_be_bytes_mod_r(&input), ark_reduce(&input));
    }

    #[test]
    fn reduce_sha256_matches_arkworks() {
        let seed = [0xAA; 32];
        let hash = anchor_lang::solana_program::hash::hash(&seed);
        let hash_bytes: [u8; 32] = hash.to_bytes();
        assert_eq!(reduce_be_bytes_mod_r(&hash_bytes), ark_reduce(&hash_bytes));
    }
}

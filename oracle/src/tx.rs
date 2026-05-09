use anchor_lang::{InstructionData, ToAccountMetas};
use anyhow::Result;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;

use crate::epoch::CommitState;
use crate::rpc::RpcProvider;
use crate::vrf::VrfOutput;

pub struct TxBuilder<'a, R: RpcProvider> {
    pub rpc: &'a R,
    pub payer: &'a Keypair,
    pub program_id: Pubkey,
    pub epoch_state_address: Pubkey,
}

impl<'a, R: RpcProvider> TxBuilder<'a, R> {
    pub fn new(
        rpc: &'a R,
        payer: &'a Keypair,
        program_id: Pubkey,
        epoch_state_address: Pubkey,
    ) -> Self {
        Self {
            rpc,
            payer,
            program_id,
            epoch_state_address,
        }
    }

    pub async fn send_commit(&self, commitment: [u8; 32]) -> Result<Signature> {
        let ix = self.build_commit_instruction(commitment);
        let blockhash = self.rpc.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[self.payer],
            blockhash,
        );
        let sig = self.rpc.send_and_confirm_transaction(&tx).await?;
        Ok(sig)
    }

    pub async fn send_reveal(&self, commit_state: &CommitState) -> Result<Signature> {
        let ix = self.build_reveal_instruction(commit_state.salt);
        let blockhash = self.rpc.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[self.payer],
            blockhash,
        );
        let sig = self.rpc.send_and_confirm_transaction(&tx).await?;
        Ok(sig)
    }

    pub fn build_reveal_instruction(&self, seed: [u8; 32]) -> Instruction {
        let accounts = randomness_beacon::accounts::OracleReveal {
            oracle: self.payer.pubkey(),
            epoch_state: self.epoch_state_address,
        };
        let ix_data = randomness_beacon::instruction::OracleReveal { seed };
        Instruction {
            program_id: self.program_id,
            accounts: accounts.to_account_metas(None),
            data: ix_data.data(),
        }
    }

    pub async fn send_finalize(&self, vrf_output: &VrfOutput) -> Result<Signature> {
        let ix = self.build_finalize_instruction(vrf_output);
        let blockhash = self.rpc.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[self.payer],
            blockhash,
        );
        let sig = self.rpc.send_and_confirm_transaction(&tx).await?;
        Ok(sig)
    }

    pub fn build_commit_instruction(&self, commitment: [u8; 32]) -> Instruction {
        let accounts = randomness_beacon::accounts::OracleCommit {
            oracle: self.payer.pubkey(),
            epoch_state: self.epoch_state_address,
        };
        let ix_data = randomness_beacon::instruction::OracleCommit { commitment };
        Instruction {
            program_id: self.program_id,
            accounts: accounts.to_account_metas(None),
            data: ix_data.data(),
        }
    }

    pub fn build_finalize_instruction(&self, vrf_output: &VrfOutput) -> Instruction {
        let accounts = randomness_beacon::accounts::FinalizeEpoch {
            oracle: self.payer.pubkey(),
            epoch_state: self.epoch_state_address,
        };
        let ix_data = randomness_beacon::instruction::FinalizeEpoch {
            vrf_output: vrf_output.output,
            proof: vrf_output.proof.clone(),
            public_inputs: vrf_output.public_inputs.clone(),
        };
        Instruction {
            program_id: self.program_id,
            accounts: accounts.to_account_metas(None),
            data: ix_data.data(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use solana_sdk::hash::Hash;

    struct MockRpc;

    #[async_trait]
    impl RpcProvider for MockRpc {
        async fn get_slot(&self) -> Result<u64> {
            Ok(0)
        }
        async fn get_account_data(&self, _pubkey: &Pubkey) -> Result<Option<Vec<u8>>> {
            Ok(None)
        }
        async fn get_latest_blockhash(&self) -> Result<Hash> {
            Ok(Hash::default())
        }
        async fn send_and_confirm_transaction(&self, _tx: &Transaction) -> Result<Signature> {
            Ok(Signature::default())
        }
    }

    #[test]
    fn build_commit_instruction_has_correct_accounts() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let commitment = [0xBB; 32];
        let ix = builder.build_commit_instruction(commitment);

        assert_eq!(ix.program_id, program_id);
        // First account is oracle (signer+writable), second is epoch_state (writable)
        assert_eq!(ix.accounts[0].pubkey, payer.pubkey());
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[0].is_writable);
        assert_eq!(ix.accounts[1].pubkey, epoch_state);
        assert!(ix.accounts[1].is_writable);
    }

    #[test]
    fn build_commit_instruction_encodes_commitment_in_data() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let commitment = [0xCC; 32];
        let ix = builder.build_commit_instruction(commitment);

        // Anchor ix data: 8-byte discriminator + 32-byte commitment
        assert_eq!(ix.data.len(), 8 + 32);
        assert_eq!(&ix.data[8..], &commitment);
    }

    #[tokio::test]
    async fn send_commit_calls_rpc() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let commitment = [0xDD; 32];
        let sig = builder.send_commit(commitment).await.unwrap();
        assert_eq!(sig, Signature::default());
    }

    #[test]
    fn build_reveal_instruction_has_correct_accounts() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let seed = [0xAA; 32];
        let ix = builder.build_reveal_instruction(seed);

        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.accounts[0].pubkey, payer.pubkey());
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[0].is_writable);
        assert_eq!(ix.accounts[1].pubkey, epoch_state);
        assert!(ix.accounts[1].is_writable);
    }

    #[test]
    fn build_reveal_instruction_encodes_salt_in_data() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let seed = [0xEE; 32];
        let ix = builder.build_reveal_instruction(seed);

        // Anchor ix data: 8-byte discriminator + 32-byte seed
        assert_eq!(ix.data.len(), 8 + 32);
        assert_eq!(&ix.data[8..], &seed);
    }

    #[tokio::test]
    async fn send_reveal_calls_rpc() {
        use crate::epoch::CommitState;

        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let commit_state = CommitState {
            epoch_id: 1,
            salt: [0xFF; 32],
        };
        let sig = builder.send_reveal(&commit_state).await.unwrap();
        assert_eq!(sig, Signature::default());
    }

    #[test]
    fn build_finalize_instruction_encodes_output_proof_and_public_inputs() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let epoch_state = Pubkey::new_unique();
        let rpc = MockRpc;

        let builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state);
        let vrf_output = VrfOutput {
            output: [0x11; 32],
            proof: vec![0x22; 256],
            public_inputs: vec![[0x33; 32], [0x11; 32]],
        };
        let ix = builder.build_finalize_instruction(&vrf_output);

        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.accounts[0].pubkey, payer.pubkey());
        assert!(ix.accounts[0].is_signer);
        assert_eq!(ix.accounts[1].pubkey, epoch_state);

        let expected = randomness_beacon::instruction::FinalizeEpoch {
            vrf_output: vrf_output.output,
            proof: vrf_output.proof,
            public_inputs: vrf_output.public_inputs,
        }
        .data();
        assert_eq!(ix.data, expected);
    }
}

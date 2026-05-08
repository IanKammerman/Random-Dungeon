use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use ecvrf_solana_program::state::VrfProofRecord;
use solana_program_test::{processor, ProgramTest};
use solana_sdk::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};

const PROOF_SOLANA: &[u8; 256] = include_bytes!("../../../artifacts/proof_solana.bin");
const PUBLIC_INPUTS_SOLANA: &[u8; 64] =
    include_bytes!("../../../artifacts/public_inputs_solana.bin");

fn public_inputs() -> [[u8; 32]; 2] {
    [
        PUBLIC_INPUTS_SOLANA[..32].try_into().unwrap(),
        PUBLIC_INPUTS_SOLANA[32..].try_into().unwrap(),
    ]
}

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Anchor's generated entrypoint uses a stricter account lifetime than the
    // generic native processor function expected by solana-program-test.
    let accounts = unsafe { std::mem::transmute::<&[AccountInfo<'_>], &[AccountInfo<'_>]>(accounts) };
    ecvrf_solana_program::entry(program_id, accounts, instruction_data)
}

#[tokio::test]
async fn local_validator_accepts_real_vrf_proof() {
    let program_id = ecvrf_solana_program::ID;
    let public_inputs = public_inputs();
    let program_test = ProgramTest::new(
        "ecvrf_solana_program",
        program_id,
        processor!(process_instruction),
    );

    let (banks_client, payer, recent_blockhash) = program_test.start().await;
    let authority = Keypair::new();

    let fund_ix = system_instruction::transfer(&payer.pubkey(), &authority.pubkey(), 1_000_000_000);
    let mut fund_tx = Transaction::new_with_payer(&[fund_ix], Some(&payer.pubkey()));
    fund_tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(fund_tx).await.unwrap();

    let (record, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[b"vrf-proof-record", authority.pubkey().as_ref()],
        &program_id,
    );

    let ix = Instruction {
        program_id,
        accounts: ecvrf_solana_program::accounts::VerifyVrfProof {
            authority: authority.pubkey(),
            record,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: ecvrf_solana_program::instruction::VerifyVrfProof {
            proof: PROOF_SOLANA.to_vec(),
            public_inputs: public_inputs.to_vec(),
        }
        .data(),
    };

    let latest_blockhash = banks_client.get_latest_blockhash().await.unwrap();
    let mut tx = Transaction::new_with_payer(&[ix], Some(&authority.pubkey()));
    tx.sign(&[&authority], latest_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    let account = banks_client.get_account(record).await.unwrap().unwrap();
    let mut data = account.data.as_slice();
    let record = VrfProofRecord::try_deserialize(&mut data).unwrap();

    assert!(record.accepted);
    assert_eq!(record.authority, authority.pubkey());
    assert_eq!(record.beta, public_inputs[1]);
    assert_eq!(record.public_inputs, public_inputs);
}

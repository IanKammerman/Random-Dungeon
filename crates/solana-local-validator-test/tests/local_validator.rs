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

const PROOF_SOLANA: [u8; 256] = [
    22, 20, 122, 32, 242, 36, 185, 165, 170, 25, 191, 197, 141, 79, 167, 158, 127, 17, 132, 157,
    207, 159, 229, 58, 117, 152, 59, 136, 168, 153, 189, 132, 27, 12, 99, 182, 96, 29, 73, 117,
    247, 194, 218, 70, 183, 191, 35, 80, 207, 209, 169, 185, 82, 69, 227, 148, 53, 109, 184, 49,
    252, 118, 200, 180, 24, 9, 196, 230, 124, 135, 88, 248, 217, 130, 251, 148, 214, 152, 232,
    129, 55, 118, 100, 87, 163, 175, 189, 218, 144, 42, 140, 83, 8, 182, 234, 232, 9, 171, 113,
    112, 169, 174, 4, 29, 86, 92, 18, 190, 134, 132, 237, 135, 111, 213, 26, 192, 35, 67, 42,
    103, 211, 92, 167, 127, 201, 179, 102, 153, 7, 86, 73, 158, 136, 130, 142, 163, 213, 174, 10,
    7, 177, 199, 217, 104, 59, 186, 89, 232, 171, 223, 159, 24, 1, 33, 4, 128, 120, 107, 39, 228,
    34, 119, 56, 17, 58, 34, 226, 218, 1, 84, 133, 47, 6, 216, 138, 99, 113, 236, 80, 171, 93,
    228, 178, 237, 252, 177, 88, 117, 238, 27, 42, 146, 0, 104, 201, 97, 183, 12, 252, 95, 222,
    86, 112, 189, 39, 138, 246, 141, 117, 117, 166, 180, 231, 218, 126, 98, 98, 67, 65, 171, 175,
    51, 122, 188, 19, 174, 189, 240, 211, 139, 230, 137, 204, 40, 164, 229, 161, 248, 168, 178,
    177, 163, 41, 153, 34, 106, 150, 65, 26, 165, 62, 146, 186, 60, 178, 90,
];

const PUBLIC_INPUTS: [[u8; 32]; 2] = [
    [
        18, 193, 106, 239, 122, 164, 21, 24, 147, 152, 238, 99, 117, 25, 182, 242, 191, 232, 74,
        124, 113, 250, 72, 245, 1, 185, 244, 140, 96, 217, 56, 63,
    ],
    [
        17, 39, 178, 169, 31, 136, 128, 208, 160, 237, 202, 14, 208, 1, 101, 51, 80, 41, 97, 111,
        84, 148, 200, 209, 91, 233, 170, 107, 216, 233, 132, 43,
    ],
];

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
            public_inputs: PUBLIC_INPUTS.to_vec(),
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
    assert_eq!(record.beta, PUBLIC_INPUTS[1]);
    assert_eq!(record.public_inputs, PUBLIC_INPUTS);
}

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use ark_bn254::{g1::G1Affine, g2::G2Affine, Bn254, Fq, Fr};
use ark_ff::{BigInteger, PrimeField};
use ark_groth16::{Groth16, Proof, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use clap::Parser;
use rand::rngs::OsRng;
use std::ops::Neg;
use vrf_circuit::VrfCircuit;
use vrf_core::{compute_vrf, fr_from_hex, PublicInputs};

#[derive(Debug, Parser)]
#[command(about = "Generate a Groth16 proof for the MVP VRF computation")]
struct Cli {
    #[arg(long)]
    sk: String,
    #[arg(long)]
    alpha: String,
    #[arg(long, default_value = "artifacts/proving_key.bin")]
    proving_key: PathBuf,
    #[arg(long, default_value = "artifacts/proof.bin")]
    proof: PathBuf,
    #[arg(long = "public-inputs", default_value = "artifacts/public_inputs.json")]
    public_inputs: PathBuf,
    #[arg(long = "solana-proof", default_value = "artifacts/proof_solana.bin")]
    solana_proof: PathBuf,
    #[arg(
        long = "solana-public-inputs",
        default_value = "artifacts/public_inputs_solana.json"
    )]
    solana_public_inputs: PathBuf,
    #[arg(
        long = "solana-public-inputs-bin",
        default_value = "artifacts/public_inputs_solana.bin"
    )]
    solana_public_inputs_bin: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sk = parse_secret(&cli.sk)?;
    prove_to_files(
        sk,
        cli.alpha.as_bytes(),
        &cli.proving_key,
        &cli.proof,
        &cli.public_inputs,
        &cli.solana_proof,
        &cli.solana_public_inputs,
        &cli.solana_public_inputs_bin,
    )
}

fn parse_secret(input: &str) -> Result<Fr> {
    if input.starts_with("0x") {
        return fr_from_hex(input);
    }
    let value = input.parse::<u64>().context("secret key must be a u64 decimal or 0x hex field element")?;
    Ok(Fr::from(value))
}

fn prove_to_files(
    sk: Fr,
    alpha: &[u8],
    proving_key_path: &PathBuf,
    proof_path: &PathBuf,
    public_inputs_path: &PathBuf,
    solana_proof_path: &PathBuf,
    solana_public_inputs_path: &PathBuf,
    solana_public_inputs_bin_path: &PathBuf,
) -> Result<()> {
    let pk_bytes = fs::read(proving_key_path)
        .with_context(|| format!("failed to read {}", proving_key_path.display()))?;
    let pk = ProvingKey::<Bn254>::deserialize_compressed(pk_bytes.as_slice())
        .context("failed to deserialize proving key")?;

    let evaluation = compute_vrf(sk, alpha);
    let circuit = VrfCircuit {
        sk: Some(sk),
        alpha_hash: Some(evaluation.alpha_hash),
        beta: Some(evaluation.beta),
    };

    let mut rng = OsRng;
    let proof = Groth16::<Bn254>::create_random_proof_with_reduction(circuit, &pk, &mut rng)
        .context("failed to create Groth16 proof")?;

    if let Some(parent) = proof_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut proof_bytes = Vec::new();
    proof
        .serialize_compressed(&mut proof_bytes)
        .context("failed to serialize proof")?;
    fs::write(proof_path, proof_bytes).with_context(|| format!("failed to write {}", proof_path.display()))?;

    let public_inputs = PublicInputs::new(evaluation.alpha_hash, evaluation.beta);
    let json = serde_json::to_string_pretty(&public_inputs).context("failed to encode public inputs")?;
    fs::write(public_inputs_path, json)
        .with_context(|| format!("failed to write {}", public_inputs_path.display()))?;

    let solana_proof = proof_to_solana_bytes(&proof);
    fs::write(solana_proof_path, solana_proof)
        .with_context(|| format!("failed to write {}", solana_proof_path.display()))?;

    let solana_inputs = public_inputs.to_solana_bytes()?;
    let solana_inputs_json =
        serde_json::to_string_pretty(&solana_inputs).context("failed to encode Solana public inputs")?;
    fs::write(solana_public_inputs_path, solana_inputs_json)
        .with_context(|| format!("failed to write {}", solana_public_inputs_path.display()))?;
    fs::write(solana_public_inputs_bin_path, public_inputs.to_solana_flat_bytes()?)
        .with_context(|| format!("failed to write {}", solana_public_inputs_bin_path.display()))?;

    println!("wrote {}", proof_path.display());
    println!("wrote {}", public_inputs_path.display());
    println!("wrote {}", solana_proof_path.display());
    println!("wrote {}", solana_public_inputs_path.display());
    println!("wrote {}", solana_public_inputs_bin_path.display());

    Ok(())
}

fn proof_to_solana_bytes(proof: &Proof<Bn254>) -> [u8; 256] {
    // groth16-solana verifies e(-A, B) * e(prepared_inputs, gamma)
    // * e(C, delta) * e(alpha, beta) == 1, so proof A is negated here.
    let proof_a = g1_to_solana_bytes(&proof.a.neg());
    let proof_b = g2_to_solana_bytes(&proof.b);
    let proof_c = g1_to_solana_bytes(&proof.c);

    let mut out = [0u8; 256];
    out[..64].copy_from_slice(&proof_a);
    out[64..192].copy_from_slice(&proof_b);
    out[192..].copy_from_slice(&proof_c);
    out
}

fn fq_to_be_bytes(value: &Fq) -> [u8; 32] {
    let bytes = (*value).into_bigint().to_bytes_be();
    let mut out = [0u8; 32];
    let start = out.len().saturating_sub(bytes.len());
    out[start..].copy_from_slice(&bytes);
    out
}

fn g1_to_solana_bytes(point: &G1Affine) -> [u8; 64] {
    let x = fq_to_be_bytes(&point.x);
    let y = fq_to_be_bytes(&point.y);
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&x);
    out[32..].copy_from_slice(&y);
    out
}

fn g2_to_solana_bytes(point: &G2Affine) -> [u8; 128] {
    let x_c1 = fq_to_be_bytes(&point.x.c1);
    let x_c0 = fq_to_be_bytes(&point.x.c0);
    let y_c1 = fq_to_be_bytes(&point.y.c1);
    let y_c0 = fq_to_be_bytes(&point.y.c0);
    let mut out = [0u8; 128];
    out[..32].copy_from_slice(&x_c1);
    out[32..64].copy_from_slice(&x_c0);
    out[64..96].copy_from_slice(&y_c1);
    out[96..].copy_from_slice(&y_c0);
    out
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use ark_groth16::{prepare_verifying_key, Groth16, Proof};
    use ark_serialize::CanonicalDeserialize;
    use ark_serialize::CanonicalSerialize;
    use groth16_solana::groth16::{Groth16Verifier, Groth16Verifyingkey};
    use rand::rngs::OsRng;
    use vrf_circuit::test_vectors::sample_circuit;

    use super::*;

    #[test]
    fn prover_creates_proof_and_public_inputs() {
        let dir = temp_dir("ecvrf-prover-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut rng = OsRng;
        let pk = Groth16::<Bn254>::generate_random_parameters_with_reduction(
            sample_circuit(),
            &mut rng,
        )
        .unwrap();

        let pk_path = dir.join("proving_key.bin");
        let proof_path = dir.join("proof.bin");
        let public_inputs_path = dir.join("public_inputs.json");
        let solana_proof_path = dir.join("proof_solana.bin");
        let solana_public_inputs_path = dir.join("public_inputs_solana.json");
        let solana_public_inputs_bin_path = dir.join("public_inputs_solana.bin");
        let mut pk_bytes = Vec::new();
        pk.serialize_compressed(&mut pk_bytes).unwrap();
        fs::write(&pk_path, pk_bytes).unwrap();

        prove_to_files(
            Fr::from(12345u64),
            b"prover test alpha",
            &pk_path,
            &proof_path,
            &public_inputs_path,
            &solana_proof_path,
            &solana_public_inputs_path,
            &solana_public_inputs_bin_path,
        )
        .unwrap();

        assert!(proof_path.exists());
        assert!(public_inputs_path.exists());
        assert_eq!(fs::read(&solana_proof_path).unwrap().len(), 256);
        assert_eq!(fs::read(&solana_public_inputs_bin_path).unwrap().len(), 64);

        let proof_bytes = fs::read(&proof_path).unwrap();
        let proof = Proof::<Bn254>::deserialize_compressed(proof_bytes.as_slice()).unwrap();
        let public_inputs: PublicInputs =
            serde_json::from_str(&fs::read_to_string(&public_inputs_path).unwrap()).unwrap();
        let inputs = public_inputs.to_field_elements().unwrap();
        let pvk = prepare_verifying_key(&pk.vk);
        assert!(Groth16::<Bn254>::verify_proof(&pvk, &proof, &inputs).unwrap());

        let proof_solana: [u8; 256] = fs::read(&solana_proof_path).unwrap().try_into().unwrap();
        let public_inputs_solana = public_inputs.to_solana_bytes().unwrap();
        let vk_ic: Vec<[u8; 64]> = pk.vk.gamma_abc_g1.iter().map(g1_to_solana_bytes).collect();
        let vk = Groth16Verifyingkey {
            nr_pubinputs: 2,
            vk_alpha_g1: g1_to_solana_bytes(&pk.vk.alpha_g1),
            vk_beta_g2: g2_to_solana_bytes(&pk.vk.beta_g2),
            vk_gamme_g2: g2_to_solana_bytes(&pk.vk.gamma_g2),
            vk_delta_g2: g2_to_solana_bytes(&pk.vk.delta_g2),
            vk_ic: &vk_ic,
        };
        let proof_a: [u8; 64] = proof_solana[..64].try_into().unwrap();
        let proof_b: [u8; 128] = proof_solana[64..192].try_into().unwrap();
        let proof_c: [u8; 64] = proof_solana[192..].try_into().unwrap();
        let mut solana_verifier =
            Groth16Verifier::new(&proof_a, &proof_b, &proof_c, &public_inputs_solana, &vk)
                .unwrap();
        solana_verifier.verify().unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        PathBuf::from(format!(
            "{}/{}-{}",
            std::env::temp_dir().display(),
            prefix,
            std::process::id()
        ))
    }
}

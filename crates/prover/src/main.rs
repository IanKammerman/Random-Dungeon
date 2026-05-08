use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use clap::Parser;
use rand::rngs::OsRng;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sk = parse_secret(&cli.sk)?;
    prove_to_files(sk, cli.alpha.as_bytes(), &cli.proving_key, &cli.proof, &cli.public_inputs)
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

    println!("wrote {}", proof_path.display());
    println!("wrote {}", public_inputs_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use ark_groth16::{prepare_verifying_key, Groth16, Proof};
    use ark_serialize::CanonicalDeserialize;
    use ark_serialize::CanonicalSerialize;
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
        let mut pk_bytes = Vec::new();
        pk.serialize_compressed(&mut pk_bytes).unwrap();
        fs::write(&pk_path, pk_bytes).unwrap();

        prove_to_files(
            Fr::from(12345u64),
            b"prover test alpha",
            &pk_path,
            &proof_path,
            &public_inputs_path,
        )
        .unwrap();

        assert!(proof_path.exists());
        assert!(public_inputs_path.exists());

        let proof_bytes = fs::read(&proof_path).unwrap();
        let proof = Proof::<Bn254>::deserialize_compressed(proof_bytes.as_slice()).unwrap();
        let public_inputs: PublicInputs =
            serde_json::from_str(&fs::read_to_string(&public_inputs_path).unwrap()).unwrap();
        let inputs = public_inputs.to_field_elements().unwrap();
        let pvk = prepare_verifying_key(&pk.vk);
        assert!(Groth16::<Bn254>::verify_proof(&pvk, &proof, &inputs).unwrap());

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

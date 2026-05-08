use std::{fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use ark_bn254::Bn254;
use ark_groth16::{prepare_verifying_key, Groth16, Proof, VerifyingKey};
use ark_serialize::CanonicalDeserialize;
use clap::Parser;
use vrf_core::PublicInputs;

#[derive(Debug, Parser)]
#[command(about = "Verify a Groth16 proof for the MVP VRF computation")]
struct Cli {
    #[arg(long, default_value = "artifacts/proof.bin")]
    proof: PathBuf,
    #[arg(long = "public-inputs", default_value = "artifacts/public_inputs.json")]
    public_inputs: PathBuf,
    #[arg(long, default_value = "artifacts/verifying_key.bin")]
    vk: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let valid = verify_from_files(&cli.proof, &cli.public_inputs, &cli.vk)?;
    if valid {
        println!("valid");
        Ok(())
    } else {
        println!("invalid");
        bail!("proof verification failed")
    }
}

fn verify_from_files(proof_path: &PathBuf, public_inputs_path: &PathBuf, vk_path: &PathBuf) -> Result<bool> {
    let vk_bytes = fs::read(vk_path).with_context(|| format!("failed to read {}", vk_path.display()))?;
    let vk = VerifyingKey::<Bn254>::deserialize_compressed(vk_bytes.as_slice())
        .context("failed to deserialize verifying key")?;

    let proof_bytes = fs::read(proof_path).with_context(|| format!("failed to read {}", proof_path.display()))?;
    let proof = Proof::<Bn254>::deserialize_compressed(proof_bytes.as_slice())
        .context("failed to deserialize proof")?;

    let public_inputs_json = fs::read_to_string(public_inputs_path)
        .with_context(|| format!("failed to read {}", public_inputs_path.display()))?;
    let public_inputs: PublicInputs =
        serde_json::from_str(&public_inputs_json).context("invalid public inputs JSON")?;
    let inputs = public_inputs.to_field_elements()?;

    let pvk = prepare_verifying_key(&vk);
    Groth16::<Bn254>::verify_proof(&pvk, &proof, &inputs).context("verification failed")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use ark_bn254::Fr;
    use ark_groth16::ProvingKey;
    use ark_serialize::CanonicalSerialize;
    use rand::rngs::OsRng;
    use vrf_circuit::{test_vectors::sample_circuit, VrfCircuit};
    use vrf_core::{compute_vrf, PublicInputs};

    use super::*;

    #[test]
    fn verifier_accepts_valid_proof() {
        let files = build_valid_fixture("ecvrf-verifier-valid");
        assert!(verify_from_files(&files.proof, &files.public_inputs, &files.vk).unwrap());
        let _ = fs::remove_dir_all(files.dir);
    }

    #[test]
    fn verifier_rejects_modified_public_input() {
        let files = build_valid_fixture("ecvrf-verifier-invalid");
        let mut public_inputs: PublicInputs =
            serde_json::from_str(&fs::read_to_string(&files.public_inputs).unwrap()).unwrap();
        let mut fields = public_inputs.to_field_elements().unwrap();
        fields[1] += Fr::from(1u64);
        public_inputs.beta = vrf_core::fr_to_hex(&fields[1]);
        fs::write(
            &files.public_inputs,
            serde_json::to_string_pretty(&public_inputs).unwrap(),
        )
        .unwrap();

        assert!(!verify_from_files(&files.proof, &files.public_inputs, &files.vk).unwrap());
        let _ = fs::remove_dir_all(files.dir);
    }

    struct FixtureFiles {
        dir: PathBuf,
        proof: PathBuf,
        public_inputs: PathBuf,
        vk: PathBuf,
    }

    fn build_valid_fixture(prefix: &str) -> FixtureFiles {
        let dir = temp_dir(prefix);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let sk = Fr::from(12345u64);
        let evaluation = compute_vrf(sk, b"verifier test alpha");
        let circuit = VrfCircuit {
            sk: Some(sk),
            alpha_hash: Some(evaluation.alpha_hash),
            beta: Some(evaluation.beta),
        };
        let mut rng = OsRng;
        let pk: ProvingKey<Bn254> =
            Groth16::<Bn254>::generate_random_parameters_with_reduction(
                sample_circuit(),
                &mut rng,
            )
            .unwrap();
        let proof = Groth16::<Bn254>::create_random_proof_with_reduction(circuit, &pk, &mut rng)
            .unwrap();

        let proof_path = dir.join("proof.bin");
        let public_inputs_path = dir.join("public_inputs.json");
        let vk_path = dir.join("verifying_key.bin");

        let mut proof_bytes = Vec::new();
        proof.serialize_compressed(&mut proof_bytes).unwrap();
        fs::write(&proof_path, proof_bytes).unwrap();

        let mut vk_bytes = Vec::new();
        pk.vk.serialize_compressed(&mut vk_bytes).unwrap();
        fs::write(&vk_path, vk_bytes).unwrap();

        let public_inputs = PublicInputs::new(evaluation.alpha_hash, evaluation.beta);
        fs::write(
            &public_inputs_path,
            serde_json::to_string_pretty(&public_inputs).unwrap(),
        )
        .unwrap();

        FixtureFiles {
            dir,
            proof: proof_path,
            public_inputs: public_inputs_path,
            vk: vk_path,
        }
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

use std::{fs, path::Path};

use anyhow::{Context, Result};
use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_relations::r1cs::ConstraintSynthesizer;
use ark_serialize::CanonicalSerialize;
use rand::rngs::OsRng;

use crate::export_vk::export_ark_vk_for_solana;

pub fn run_local_random_setup<C>(circuit: C, artifacts_dir: &Path) -> Result<()>
where
    C: ConstraintSynthesizer<Fr>,
{
    // SECURITY: This setup mode is insecure for production. Whoever runs it
    // could retain toxic waste. Use only for local development and testing.
    let mut rng = OsRng;
    fs::create_dir_all(artifacts_dir)
        .with_context(|| format!("failed to create {}", artifacts_dir.display()))?;

    let pk = Groth16::<Bn254>::generate_random_parameters_with_reduction(circuit, &mut rng)
        .context("failed to generate Groth16 parameters")?;
    let vk = pk.vk.clone();

    let pk_path = artifacts_dir.join("proving_key.bin");
    let vk_path = artifacts_dir.join("verifying_key.bin");
    let solana_vk_path = artifacts_dir.join("verifying_key_solana.rs");

    let mut pk_bytes = Vec::new();
    pk.serialize_compressed(&mut pk_bytes)
        .context("failed to serialize proving key")?;
    fs::write(&pk_path, pk_bytes).with_context(|| format!("failed to write {}", pk_path.display()))?;

    let mut vk_bytes = Vec::new();
    vk.serialize_compressed(&mut vk_bytes)
        .context("failed to serialize verifying key")?;
    fs::write(&vk_path, vk_bytes).with_context(|| format!("failed to write {}", vk_path.display()))?;

    export_ark_vk_for_solana(&vk, &solana_vk_path)?;

    println!("wrote {}", pk_path.display());
    println!("wrote {}", vk_path.display());
    println!("wrote {}", solana_vk_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use vrf_circuit::test_vectors::sample_circuit;

    use super::run_local_random_setup;

    #[test]
    fn local_random_setup_generates_artifacts() {
        let dir = PathBuf::from(format!(
            "{}/ecvrf-setup-test-{}",
            std::env::temp_dir().display(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);

        run_local_random_setup(sample_circuit(), &dir).unwrap();

        assert!(dir.join("proving_key.bin").exists());
        assert!(dir.join("verifying_key.bin").exists());
        assert!(dir.join("verifying_key_solana.rs").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

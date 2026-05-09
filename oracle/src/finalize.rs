use std::path::Path;

use anyhow::{bail, Context, Result};
use solana_sdk::signature::Signature;
use tokio::process::Command;
use tracing::debug;

use crate::rpc::RpcProvider;
use crate::tx::TxBuilder;
use crate::vrf::{OracleVrf, VrfOutput};

pub async fn run_finalize<R: RpcProvider>(
    tx_builder: &TxBuilder<'_, R>,
    vrf: &OracleVrf,
    vrf_secret_hex: &str,
    prover_binary: &Path,
    proving_key_path: &Path,
    entropy_seed: &[u8; 32],
) -> Result<Signature> {
    let eval = vrf.evaluate(entropy_seed);

    let alpha_hex = format!("0x{}", hex::encode(entropy_seed));

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir for prover output")?;
    let proof_path = tmp_dir.path().join("proof_solana.bin");
    let public_inputs_path = tmp_dir.path().join("public_inputs_solana.bin");

    let output = Command::new(prover_binary)
        .arg("--sk")
        .arg(vrf_secret_hex)
        .arg("--alpha-hex")
        .arg(&alpha_hex)
        .arg("--proving-key")
        .arg(proving_key_path)
        .arg("--solana-proof")
        .arg(&proof_path)
        .arg("--solana-public-inputs-bin")
        .arg(&public_inputs_path)
        .arg("--proof")
        .arg(tmp_dir.path().join("proof.bin"))
        .arg("--public-inputs")
        .arg(tmp_dir.path().join("public_inputs.json"))
        .arg("--solana-public-inputs")
        .arg(tmp_dir.path().join("public_inputs_solana.json"))
        .output()
        .await
        .context("failed to spawn prover binary")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("prover exited with {}: {}", output.status, stderr.trim());
    }

    debug!("prover completed successfully");

    let proof_bytes = tokio::fs::read(&proof_path)
        .await
        .context("failed to read proof_solana.bin from prover output")?;
    if proof_bytes.len() != 256 {
        bail!(
            "proof_solana.bin has {} bytes, expected 256",
            proof_bytes.len()
        );
    }

    let pi_bytes = tokio::fs::read(&public_inputs_path)
        .await
        .context("failed to read public_inputs_solana.bin from prover output")?;
    if pi_bytes.len() != 64 {
        bail!(
            "public_inputs_solana.bin has {} bytes, expected 64",
            pi_bytes.len()
        );
    }

    let alpha_hash_bytes: [u8; 32] = pi_bytes[..32].try_into().unwrap();
    let beta_bytes: [u8; 32] = pi_bytes[32..64].try_into().unwrap();

    let expected_alpha_hash = vrf_core::hash::fr_to_be_bytes(&eval.alpha_hash);
    let expected_beta = vrf_core::hash::fr_to_be_bytes(&eval.beta);

    if alpha_hash_bytes != expected_alpha_hash {
        bail!(
            "prover alpha_hash mismatch: oracle computed 0x{}, prover produced 0x{}",
            hex::encode(expected_alpha_hash),
            hex::encode(alpha_hash_bytes)
        );
    }
    if beta_bytes != expected_beta {
        bail!(
            "prover beta mismatch: oracle computed 0x{}, prover produced 0x{}",
            hex::encode(expected_beta),
            hex::encode(beta_bytes)
        );
    }

    let vrf_output = VrfOutput {
        output: beta_bytes,
        proof: proof_bytes,
        public_inputs: vec![alpha_hash_bytes, beta_bytes],
    };

    let sig = tx_builder.send_finalize(&vrf_output).await?;
    Ok(sig)
}

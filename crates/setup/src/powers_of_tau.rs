use std::{fs, path::Path};

use anyhow::{bail, Context, Result};

use crate::export_vk::export_snarkjs_vk_json_for_solana;

pub fn import_ceremony(zkey: &Path, vk_json: &Path, out: &Path) -> Result<()> {
    if !zkey.exists() {
        bail!("zkey file does not exist: {}", zkey.display());
    }

    let json = fs::read_to_string(vk_json)
        .with_context(|| format!("failed to read {}", vk_json.display()))?;
    let vk: serde_json::Value =
        serde_json::from_str(&json).with_context(|| format!("invalid JSON in {}", vk_json.display()))?;

    export_snarkjs_vk_json_for_solana(&vk, out)?;
    println!("imported ceremony verifying key into {}", out.display());
    Ok(())
}


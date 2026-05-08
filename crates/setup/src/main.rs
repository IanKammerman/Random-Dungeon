use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use vrf_circuit::test_vectors::sample_circuit;

mod export_vk;
mod local_random;
mod powers_of_tau;

#[derive(Debug, Parser)]
#[command(about = "ECVRF SNARK setup utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    LocalRandom {
        #[arg(long, default_value = "artifacts")]
        artifacts: PathBuf,
    },
    ImportCeremony {
        #[arg(long)]
        zkey: PathBuf,
        #[arg(long = "vk-json")]
        vk_json: PathBuf,
        #[arg(long, default_value = "artifacts/verifying_key_solana.rs")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::LocalRandom { artifacts } => {
            let circuit = sample_circuit();
            local_random::run_local_random_setup(circuit, &artifacts)?;
        }
        Command::ImportCeremony { zkey, vk_json, out } => {
            powers_of_tau::import_ceremony(&zkey, &vk_json, &out)?;
        }
    }
    Ok(())
}

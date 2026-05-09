use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use oracle::entropy::seed::{archive, build_entropy_bundle};
use oracle::epoch::{CommitState, EpochMonitor};
use oracle::rpc::SolanaRpc;
use oracle::tx::TxBuilder;
use randomness_beacon::EpochPhase;

#[derive(Parser)]
#[command(about = "Random Dungeon oracle service")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Long-running service that monitors epochs and submits commit/reveal (default)
    Run,
    /// Initialize one epoch account with this wallet as the oracle authority
    InitEpoch {
        #[arg(long)]
        epoch_id: u64,
        #[arg(long)]
        commit_deadline_slot: u64,
        #[arg(long)]
        reveal_deadline_slot: u64,
        #[arg(long)]
        finalize_deadline_slot: u64,
    },
    /// Single-pass: commit and reveal for one epoch, then exit
    CommitOnce,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Run);

    match command {
        Command::Run => cmd_run().await,
        Command::InitEpoch {
            epoch_id,
            commit_deadline_slot,
            reveal_deadline_slot,
            finalize_deadline_slot,
        } => {
            cmd_init_epoch(
                epoch_id,
                commit_deadline_slot,
                reveal_deadline_slot,
                finalize_deadline_slot,
            )
            .await
        }
        Command::CommitOnce => cmd_commit_once().await,
    }
}

async fn cmd_init_epoch(
    epoch_id: u64,
    commit_deadline_slot: u64,
    reveal_deadline_slot: u64,
    finalize_deadline_slot: u64,
) -> Result<()> {
    ensure_deadlines_ordered(
        commit_deadline_slot,
        reveal_deadline_slot,
        finalize_deadline_slot,
    )?;

    let (rpc_url, keypair_path, program_id) = read_basic_env()?;
    let payer = read_oracle_keypair(&keypair_path)?;
    let rpc_payer = read_oracle_keypair(&keypair_path)?;
    let rpc = SolanaRpc {
        client: RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed()),
        payer: rpc_payer,
        program_id,
    };

    let (epoch_state_address, _bump) =
        Pubkey::find_program_address(&[b"epoch", &epoch_id.to_le_bytes()], &program_id);
    let tx_builder = TxBuilder::new(&rpc, &payer, program_id, epoch_state_address);
    let sig = tx_builder
        .send_initialize_epoch(
            epoch_id,
            commit_deadline_slot,
            reveal_deadline_slot,
            finalize_deadline_slot,
        )
        .await?;

    info!(
        %sig,
        %epoch_state_address,
        epoch_id,
        commit_deadline_slot,
        reveal_deadline_slot,
        finalize_deadline_slot,
        "initialize_epoch transaction confirmed"
    );
    Ok(())
}

async fn cmd_run() -> Result<()> {
    let cfg = oracle::config::Config::from_env()?;

    let payer = read_oracle_keypair(&cfg.keypair_path)?;
    let rpc_payer = read_oracle_keypair(&cfg.keypair_path)?;
    let rpc = SolanaRpc::new(&cfg, rpc_payer);

    let epoch_id = std::env::var("EPOCH_ID")
        .context("EPOCH_ID not set")?
        .parse::<u64>()
        .context("EPOCH_ID is not a valid u64")?;

    oracle::runner::run_loop(
        &rpc,
        &payer,
        cfg.program_id,
        epoch_id,
        &cfg.vrf,
        &cfg.vrf_secret_hex,
        &cfg.prover_binary_path,
        &cfg.proving_key_path,
    )
    .await
}

async fn cmd_commit_once() -> Result<()> {
    let cfg = oracle::config::Config::from_env()?;

    let payer = read_oracle_keypair(&cfg.keypair_path)?;
    let rpc_payer = read_oracle_keypair(&cfg.keypair_path)?;
    let rpc = SolanaRpc::new(&cfg, rpc_payer);

    let epoch_id = std::env::var("EPOCH_ID")
        .context("EPOCH_ID not set")?
        .parse::<u64>()
        .context("EPOCH_ID is not a valid u64")?;

    let (epoch_state_address, _bump) =
        Pubkey::find_program_address(&[b"epoch", &epoch_id.to_le_bytes()], &cfg.program_id);

    let monitor = EpochMonitor::new(&rpc, epoch_state_address);

    let phase = monitor.current_phase().await?;
    match phase {
        Some(EpochPhase::Commit) => {}
        Some(other) => {
            warn!(?other, "epoch is not in commit phase, exiting");
            return Ok(());
        }
        None => {
            warn!("epoch state account does not exist yet, exiting");
            return Ok(());
        }
    }

    info!(
        epoch_id,
        "epoch is in commit phase, generating salt and submitting commitment"
    );

    let commit_state = CommitState::new(epoch_id);
    let commitment = commit_state.commitment();

    let tx_builder = TxBuilder::new(&rpc, &payer, cfg.program_id, epoch_state_address);
    let sig = tx_builder.send_commit(commitment).await?;

    info!(%sig, "oracle_commit transaction confirmed");

    info!(epoch_id, "waiting for reveal phase");
    monitor.wait_for_phase(EpochPhase::Reveal).await?;

    info!(epoch_id, "building entropy bundle");
    let bundle = build_entropy_bundle(epoch_id).await?;
    archive(&bundle, Path::new("oracle/archive"))?;

    let sig = tx_builder
        .send_reveal(&commit_state, bundle.manifest_hash)
        .await?;

    info!(
        %sig,
        manifest_hash = %hex::encode(bundle.manifest_hash),
        seed = %hex::encode(oracle::epoch::oracle_seed(&commit_state.salt, &bundle.manifest_hash)),
        "oracle_reveal transaction confirmed"
    );

    Ok(())
}

fn read_oracle_keypair(path: &Path) -> Result<Keypair> {
    read_keypair_file(path)
        .map_err(|err| anyhow::anyhow!("failed to read keypair from {:?}: {err}", path))
}

fn read_basic_env() -> Result<(String, PathBuf, Pubkey)> {
    let rpc_url = std::env::var("SOLANA_RPC_URL").context("SOLANA_RPC_URL not set")?;
    let keypair_path = std::env::var("ORACLE_KEYPAIR_PATH")
        .map(PathBuf::from)
        .context("ORACLE_KEYPAIR_PATH not set")?;
    let program_id = std::env::var("PROGRAM_ID")
        .context("PROGRAM_ID not set")?
        .parse::<Pubkey>()
        .context("PROGRAM_ID is not a valid pubkey")?;
    Ok((rpc_url, keypair_path, program_id))
}

fn ensure_deadlines_ordered(
    commit_deadline_slot: u64,
    reveal_deadline_slot: u64,
    finalize_deadline_slot: u64,
) -> Result<()> {
    if !(commit_deadline_slot < reveal_deadline_slot
        && reveal_deadline_slot < finalize_deadline_slot)
    {
        anyhow::bail!(
            "deadline slots must satisfy commit < reveal < finalize, got {} < {} < {}",
            commit_deadline_slot,
            reveal_deadline_slot,
            finalize_deadline_slot
        );
    }
    Ok(())
}

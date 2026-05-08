use anyhow::{Context, Result};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use oracle::epoch::{CommitState, EpochMonitor};
use oracle::rpc::SolanaRpc;
use oracle::tx::TxBuilder;
use randomness_beacon::EpochPhase;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = oracle::config::Config::from_env()?;

    let keypair_bytes = std::fs::read(&cfg.keypair_path)
        .with_context(|| format!("failed to read keypair from {:?}", cfg.keypair_path))?;
    let payer = Keypair::from_bytes(&keypair_bytes)
        .context("invalid keypair bytes")?;

    let rpc = SolanaRpc::new(&cfg, Keypair::from_bytes(&keypair_bytes)?);

    let epoch_id = std::env::var("EPOCH_ID")
        .context("EPOCH_ID not set")?
        .parse::<u64>()
        .context("EPOCH_ID is not a valid u64")?;

    let (epoch_state_address, _bump) = Pubkey::find_program_address(
        &[b"epoch", &epoch_id.to_le_bytes()],
        &cfg.program_id,
    );

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

    info!(epoch_id, "epoch is in commit phase, generating salt and submitting commitment");

    let commit_state = CommitState::new(epoch_id);
    let commitment = commit_state.commitment();

    let tx_builder = TxBuilder::new(&rpc, &payer, cfg.program_id, epoch_state_address);
    let sig = tx_builder.send_commit(commitment).await?;

    info!(%sig, "oracle_commit transaction confirmed");

    info!(epoch_id, "waiting for reveal phase");
    monitor.wait_for_phase(EpochPhase::Reveal).await?;

    info!(epoch_id, "epoch is in reveal phase, submitting salt");
    let sig = tx_builder.send_reveal(&commit_state).await?;

    info!(%sig, "oracle_reveal transaction confirmed");

    Ok(())
}
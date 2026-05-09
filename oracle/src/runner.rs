// Manual testing against solana-test-validator:
//
// 1. Start a local validator:
//      solana-test-validator --reset
//
// 2. Deploy the program:
//      anchor deploy --provider.cluster localnet
//
// 3. Initialize an epoch (via a script or anchor test that calls initialize_epoch)
//
// 4. Run the oracle:
//      SOLANA_RPC_URL=http://localhost:8899 \
//      ORACLE_KEYPAIR_PATH=~/.config/solana/id.json \
//      PROGRAM_ID=9Trpfw7P4YzbaaRQYDS5fmnsAGie5JLQ1FjcgzgJfDq9 \
//      ORACLE_VRF_SECRET=0x... \
//      EPOCH_ID=1 \
//      cargo run -p oracle -- run
//
// 5. Observe logs: the oracle should detect phases, commit, wait, reveal, then
//    generate a proof and submit finalize_epoch.
//
// There is no automated integration test for the long-running loop because it
// requires a real validator with advancing slots. The per-iteration decision
// logic is tested via unit tests on `decide_action`.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use tracing::{error, info, warn};

use crate::epoch::{CommitState, EpochMonitor};
use crate::finalize;
use crate::rpc::RpcProvider;
use crate::tx::TxBuilder;
use crate::vrf::OracleVrf;
use randomness_beacon::EpochPhase;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SendCommit,
    SendReveal,
    SendFinalize,
    WaitForNextEpoch,
    Idle,
}

pub fn decide_action(
    phase: Option<EpochPhase>,
    has_committed: bool,
    has_finalized: bool,
) -> Action {
    match phase {
        Some(EpochPhase::Commit) => {
            if has_committed {
                Action::Idle
            } else {
                Action::SendCommit
            }
        }
        Some(EpochPhase::Reveal) => {
            if has_committed {
                Action::SendReveal
            } else {
                Action::WaitForNextEpoch
            }
        }
        Some(EpochPhase::Finalize) => {
            if has_finalized {
                Action::Idle
            } else {
                Action::SendFinalize
            }
        }
        Some(EpochPhase::Closed) | None => Action::WaitForNextEpoch,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_loop<R: RpcProvider>(
    rpc: &R,
    payer: &Keypair,
    program_id: Pubkey,
    epoch_id: u64,
    vrf: &OracleVrf,
    vrf_secret_hex: &str,
    prover_binary: &Path,
    proving_key_path: &Path,
) -> Result<()> {
    let (epoch_state_address, _bump) =
        Pubkey::find_program_address(&[b"epoch", &epoch_id.to_le_bytes()], &program_id);

    let monitor = EpochMonitor::new(rpc, epoch_state_address);
    let tx_builder = TxBuilder::new(rpc, payer, program_id, epoch_state_address);

    let mut commit_state: Option<CommitState> = None;
    let mut has_finalized = false;

    info!(epoch_id, "oracle service started, monitoring epoch");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("received SIGINT, shutting down gracefully");
                return Ok(());
            }
            _ = tokio::time::sleep(Duration::ZERO) => {}
        }

        let phase = match tokio::time::timeout(RPC_TIMEOUT, monitor.current_phase()).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                error!(error = %e, "RPC error reading phase, retrying");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
            Err(_) => {
                error!("RPC call timed out reading phase, retrying");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

        let action = decide_action(phase, commit_state.is_some(), has_finalized);

        match action {
            Action::SendCommit => {
                let cs = CommitState::new(epoch_id);
                let commitment = cs.commitment();
                info!(epoch_id, "commit phase: submitting commitment");

                match tokio::time::timeout(RPC_TIMEOUT, tx_builder.send_commit(commitment)).await {
                    Ok(Ok(sig)) => {
                        info!(%sig, "oracle_commit confirmed");
                        commit_state = Some(cs);
                    }
                    Ok(Err(e)) => {
                        error!(error = %e, "failed to send commit, will retry");
                    }
                    Err(_) => {
                        error!("commit transaction timed out, will retry");
                    }
                }
            }
            Action::SendReveal => {
                let cs = commit_state
                    .as_ref()
                    .expect("SendReveal requires commit_state");
                info!(epoch_id, "reveal phase: submitting salt");

                match tokio::time::timeout(RPC_TIMEOUT, tx_builder.send_reveal(cs)).await {
                    Ok(Ok(sig)) => {
                        info!(%sig, "oracle_reveal confirmed");
                        commit_state = None;
                    }
                    Ok(Err(e)) => {
                        error!(error = %e, "failed to send reveal, will retry");
                    }
                    Err(_) => {
                        error!("reveal transaction timed out, will retry");
                    }
                }
            }
            Action::SendFinalize => {
                info!(epoch_id, "finalize phase: generating proof and submitting");

                let epoch_state = match monitor.read_epoch_state().await {
                    Ok(Some(s)) => s,
                    Ok(None) => {
                        error!("epoch state disappeared during finalize");
                        tokio::time::sleep(POLL_INTERVAL).await;
                        continue;
                    }
                    Err(e) => {
                        error!(error = %e, "failed to read epoch state for finalize");
                        tokio::time::sleep(POLL_INTERVAL).await;
                        continue;
                    }
                };

                match finalize::run_finalize(
                    &tx_builder,
                    vrf,
                    vrf_secret_hex,
                    prover_binary,
                    proving_key_path,
                    &epoch_state.entropy_seed,
                )
                .await
                {
                    Ok(sig) => {
                        info!(%sig, "finalize_epoch confirmed");
                        has_finalized = true;
                    }
                    Err(e) => {
                        error!(error = %e, "finalize failed, will retry");
                    }
                }
            }
            Action::WaitForNextEpoch => {
                if commit_state.is_none() && phase.is_some() && phase != Some(EpochPhase::Closed) {
                    warn!(epoch_id, "missed this epoch's commit window, waiting for next epoch");
                }
                info!(epoch_id, "epoch closed or not initialized, waiting for next epoch");
                // In a full implementation this would scan for the next epoch PDA.
                // For now, exit cleanly since we only handle a single epoch.
                return Ok(());
            }
            Action::Idle => {}
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_phase_no_prior_commit_sends_commit() {
        assert_eq!(
            decide_action(Some(EpochPhase::Commit), false, false),
            Action::SendCommit
        );
    }

    #[test]
    fn commit_phase_already_committed_idles() {
        assert_eq!(
            decide_action(Some(EpochPhase::Commit), true, false),
            Action::Idle
        );
    }

    #[test]
    fn reveal_phase_with_commit_sends_reveal() {
        assert_eq!(
            decide_action(Some(EpochPhase::Reveal), true, false),
            Action::SendReveal
        );
    }

    #[test]
    fn reveal_phase_without_commit_waits_for_next_epoch() {
        assert_eq!(
            decide_action(Some(EpochPhase::Reveal), false, false),
            Action::WaitForNextEpoch
        );
    }

    #[test]
    fn finalize_phase_not_yet_finalized_sends_finalize() {
        assert_eq!(
            decide_action(Some(EpochPhase::Finalize), false, false),
            Action::SendFinalize
        );
        assert_eq!(
            decide_action(Some(EpochPhase::Finalize), true, false),
            Action::SendFinalize
        );
    }

    #[test]
    fn finalize_phase_already_finalized_idles() {
        assert_eq!(
            decide_action(Some(EpochPhase::Finalize), false, true),
            Action::Idle
        );
        assert_eq!(
            decide_action(Some(EpochPhase::Finalize), true, true),
            Action::Idle
        );
    }

    #[test]
    fn closed_phase_waits_for_next_epoch() {
        assert_eq!(
            decide_action(Some(EpochPhase::Closed), false, false),
            Action::WaitForNextEpoch
        );
        assert_eq!(
            decide_action(Some(EpochPhase::Closed), true, false),
            Action::WaitForNextEpoch
        );
    }

    #[test]
    fn no_epoch_waits_for_next_epoch() {
        assert_eq!(decide_action(None, false, false), Action::WaitForNextEpoch);
        assert_eq!(decide_action(None, true, false), Action::WaitForNextEpoch);
    }
}

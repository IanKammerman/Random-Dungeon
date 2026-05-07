mod config;
mod entropy;
mod epoch;
mod error;
mod rpc;
mod tx;
mod vrf;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let _cfg = config::Config::from_env()?;

    // TODO: main oracle loop
    Ok(())
}

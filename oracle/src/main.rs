use anyhow::Result;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "entropy-once" {
        let epoch: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let bundle = oracle::entropy::seed::build_entropy_bundle(epoch).await?;
        println!("epoch          = {}", bundle.epoch);
        println!("manifest_hash  = {}", hex::encode(bundle.manifest_hash));
        println!("seed           = {}", hex::encode(bundle.seed));
        oracle::entropy::seed::archive(&bundle, Path::new("oracle/archive"))?;
        println!("archived to oracle/archive/{}", bundle.epoch);
        return Ok(());
    }

    // existing main loop...
    let _cfg = oracle::config::Config::from_env()?;
    Ok(())
}
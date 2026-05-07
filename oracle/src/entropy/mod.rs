pub mod noaa;
pub mod usgs;

use anyhow::Result;

pub struct EntropySample {
    pub source: &'static str,
    pub timestamp: i64,
    pub payload: Vec<u8>,
}

#[async_trait::async_trait]
pub trait EntropySource: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch(&self) -> Result<EntropySample>;
}

pub async fn gather_entropy(sources: &[Box<dyn EntropySource>]) -> Result<Vec<EntropySample>> {
    let mut samples = Vec::with_capacity(sources.len());
    for source in sources {
        let sample = source.fetch().await?;
        samples.push(sample);
    }
    Ok(samples)
}

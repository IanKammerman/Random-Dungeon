use anyhow::Result;
use reqwest::Client;

use super::{EntropySample, EntropySource};

pub struct NoaaSource {
    pub client: Client,
}

impl NoaaSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl EntropySource for NoaaSource {
    fn name(&self) -> &'static str {
        "noaa"
    }

    async fn fetch(&self) -> Result<EntropySample> {
        // TODO: fetch atmospheric/weather data from NOAA API
        todo!()
    }
}

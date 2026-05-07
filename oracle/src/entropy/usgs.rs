use anyhow::Result;
use reqwest::Client;

use super::{EntropySample, EntropySource};

pub struct UsgsSource {
    pub client: Client,
}

impl UsgsSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl EntropySource for UsgsSource {
    fn name(&self) -> &'static str {
        "usgs"
    }

    async fn fetch(&self) -> Result<EntropySample> {
        // TODO: fetch seismic data from USGS earthquake API
        todo!()
    }
}

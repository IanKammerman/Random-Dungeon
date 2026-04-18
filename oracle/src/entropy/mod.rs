// oracle/src/entropy/mod.rs
//
// Entropy source abstractions for the randomness beacon oracle.
//
// TODO: planned submodules
//   pub mod noaa;     // weather / atmospheric data from NOAA
//   pub mod nasa;     // space / solar data from NASA APIs
//   pub mod usgs;     // seismic data from USGS
//   pub mod sports;   // public sports scores / outcomes
//
// TODO: shared interface to be implemented by each source.
//
//   pub struct EntropySample {
//       pub source: &'static str,   // human-readable source identifier
//       pub timestamp: i64,         // unix seconds when the sample was observed
//       pub payload: Vec<u8>,       // raw bytes contributed to the entropy pool
//   }
//
//   #[async_trait::async_trait]
//   pub trait EntropySource {
//       fn name(&self) -> &'static str;
//       async fn fetch(&self) -> anyhow::Result<EntropySample>;
//   }

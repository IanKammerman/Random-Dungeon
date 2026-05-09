use anyhow::{Context, Result};
use ark_bn254::Fr;
use vrf_core::{compute_vrf, fr_from_hex, VrfEvaluation};

pub struct VrfOutput {
    pub output: [u8; 32],
    pub proof: Vec<u8>,
    pub public_inputs: Vec<[u8; 32]>,
}

pub struct OracleVrf {
    sk: Fr,
}

impl OracleVrf {
    pub fn from_env() -> Result<Self> {
        let hex = std::env::var("ORACLE_VRF_SECRET")
            .context("ORACLE_VRF_SECRET not set")?;
        let sk = fr_from_hex(&hex)
            .context("ORACLE_VRF_SECRET is not a valid BN254 scalar")?;
        Ok(Self { sk })
    }

    pub fn evaluate(&self, alpha: &[u8]) -> VrfEvaluation {
        compute_vrf(self.sk, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vrf_core::serialize::fr_to_hex;

    fn test_vrf(hex_sk: &str) -> OracleVrf {
        let sk = fr_from_hex(hex_sk).unwrap();
        OracleVrf { sk }
    }

    #[test]
    fn fixed_secret_produces_expected_output() {
        let vrf = test_vrf("0x0000000000000000000000000000000000000000000000000000000000000007");
        let eval = vrf.evaluate(b"test-alpha");
        assert_eq!(
            fr_to_hex(&eval.alpha_hash),
            "0x2b3da48fc8e10085287fa1077163acf5216e0d51391e9627a0d060124aeac881"
        );
        assert_eq!(
            fr_to_hex(&eval.beta),
            "0x1e58c631a05b97db15600ad366663fbd3210a08e77b1523c2664f76a776a8cc7"
        );
    }

    #[test]
    fn evaluate_is_deterministic() {
        let vrf = test_vrf("0x3039");
        let first = vrf.evaluate(b"epoch-7");
        let second = vrf.evaluate(b"epoch-7");
        assert_eq!(first, second);
    }

    #[test]
    fn evaluate_changes_with_alpha() {
        let vrf = test_vrf("0x3039");
        let first = vrf.evaluate(b"epoch-7");
        let second = vrf.evaluate(b"epoch-8");
        assert_ne!(first.beta, second.beta);
    }

    #[test]
    fn from_env_reads_variable() {
        std::env::set_var("ORACLE_VRF_SECRET", "0x3039");
        let vrf = OracleVrf::from_env().unwrap();
        let eval = vrf.evaluate(b"test");
        assert_eq!(eval, compute_vrf(Fr::from(12345u64), b"test"));
        std::env::remove_var("ORACLE_VRF_SECRET");
    }

    #[test]
    fn from_env_rejects_invalid_hex() {
        std::env::set_var("ORACLE_VRF_SECRET", "not_hex_at_all");
        assert!(OracleVrf::from_env().is_err());
        std::env::remove_var("ORACLE_VRF_SECRET");
    }

    #[test]
    fn from_env_missing_var_errors() {
        std::env::remove_var("ORACLE_VRF_SECRET");
        assert!(OracleVrf::from_env().is_err());
    }
}

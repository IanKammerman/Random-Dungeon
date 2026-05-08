use ark_bn254::Fr;

use crate::hash::{alpha_to_fr, field_hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VrfEvaluation {
    pub alpha_hash: Fr,
    pub beta: Fr,
}

pub fn compute_beta(sk: Fr, alpha_hash: Fr) -> Fr {
    let h = field_hash(alpha_hash);
    let gamma = sk * h;
    field_hash(gamma)
}

pub fn compute_vrf(sk: Fr, alpha: impl AsRef<[u8]>) -> VrfEvaluation {
    let alpha_hash = alpha_to_fr(alpha);
    let beta = compute_beta(sk, alpha_hash);
    VrfEvaluation { alpha_hash, beta }
}

#[cfg(test)]
mod tests {
    use ark_bn254::Fr;

    use super::compute_vrf;

    #[test]
    fn deterministic_for_same_secret_and_alpha() {
        let sk = Fr::from(12345u64);
        let first = compute_vrf(sk, b"epoch-7");
        let second = compute_vrf(sk, b"epoch-7");
        assert_eq!(first, second);
    }

    #[test]
    fn different_alpha_changes_beta() {
        let sk = Fr::from(12345u64);
        let first = compute_vrf(sk, b"epoch-7");
        let second = compute_vrf(sk, b"epoch-8");
        assert_ne!(first.beta, second.beta);
    }
}


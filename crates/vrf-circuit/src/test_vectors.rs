use ark_bn254::Fr;
use vrf_core::compute_vrf;

use crate::VrfCircuit;

pub fn sample_circuit() -> VrfCircuit<Fr> {
    let sk = Fr::from(12345u64);
    let evaluation = compute_vrf(sk, b"sample alpha");
    VrfCircuit {
        sk: Some(sk),
        alpha_hash: Some(evaluation.alpha_hash),
        beta: Some(evaluation.beta),
    }
}

#[cfg(test)]
mod tests {
    use ark_bn254::Fr;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
    use vrf_core::compute_vrf;

    use crate::VrfCircuit;

    #[test]
    fn circuit_accepts_correct_witness() {
        let circuit = super::sample_circuit();
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn circuit_rejects_wrong_beta() {
        let sk = Fr::from(12345u64);
        let evaluation = compute_vrf(sk, b"sample alpha");
        let circuit = VrfCircuit {
            sk: Some(sk),
            alpha_hash: Some(evaluation.alpha_hash),
            beta: Some(evaluation.beta + Fr::from(1u64)),
        };
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }
}

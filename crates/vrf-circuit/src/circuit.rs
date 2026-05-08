use ark_ff::PrimeField;
use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget, fields::fp::FpVar};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

use crate::gadgets::field_hash_var;

#[derive(Clone, Debug, Default)]
pub struct VrfCircuit<F: PrimeField> {
    pub sk: Option<F>,
    pub alpha_hash: Option<F>,
    pub beta: Option<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for VrfCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let sk = FpVar::new_witness(cs.clone(), || self.sk.ok_or(SynthesisError::AssignmentMissing))?;
        let alpha_hash = FpVar::new_input(cs.clone(), || {
            self.alpha_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let beta = FpVar::new_input(cs, || self.beta.ok_or(SynthesisError::AssignmentMissing))?;

        let h = field_hash_var(&alpha_hash)?;
        let gamma = sk * h;
        let beta_computed = field_hash_var(&gamma)?;

        beta_computed.enforce_equal(&beta)
    }
}


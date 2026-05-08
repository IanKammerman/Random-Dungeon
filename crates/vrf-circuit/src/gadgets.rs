use ark_ff::PrimeField;
use ark_r1cs_std::{fields::fp::FpVar, prelude::*};
use ark_relations::r1cs::SynthesisError;

pub fn field_hash_var<F: PrimeField>(x: &FpVar<F>) -> Result<FpVar<F>, SynthesisError> {
    let y = x + FpVar::constant(F::from(5u64));
    let y2 = y.square()?;
    let y4 = y2.square()?;
    let y5 = y4 * y;
    let linear = x.clone() * FpVar::constant(F::from(7u64));
    Ok(y5 + linear + FpVar::constant(F::from(42u64)))
}

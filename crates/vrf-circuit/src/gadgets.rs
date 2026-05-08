use ark_crypto_primitives::sponge::{
    constraints::CryptographicSpongeVar, poseidon::constraints::PoseidonSpongeVar,
};
use ark_ff::PrimeField;
use ark_r1cs_std::{fields::fp::FpVar, R1CSVar};
use ark_relations::r1cs::SynthesisError;
use vrf_core::poseidon_config;

pub fn field_hash_var<F: PrimeField>(x: &FpVar<F>) -> Result<FpVar<F>, SynthesisError> {
    let params = poseidon_config::<F>();
    let mut sponge = PoseidonSpongeVar::<F>::new(x.cs(), &params);
    sponge.absorb(&vec![x.clone()])?;
    Ok(sponge.squeeze_field_elements(1)?[0].clone())
}

use ark_bn254::Fr;
use ark_crypto_primitives::sponge::{
    poseidon::{find_poseidon_ark_and_mds, PoseidonConfig, PoseidonSponge},
    CryptographicSponge, FieldBasedCryptographicSponge,
};
use ark_ff::{BigInteger, PrimeField};
use sha2::{Digest, Sha256};

pub fn alpha_to_fr(alpha: impl AsRef<[u8]>) -> Fr {
    let digest = Sha256::digest(alpha.as_ref());
    Fr::from_be_bytes_mod_order(&digest)
}

pub fn field_hash(x: Fr) -> Fr {
    let params = poseidon_config::<Fr>();
    let mut sponge = PoseidonSponge::<Fr>::new(&params);
    sponge.absorb(&vec![x]);
    sponge.squeeze_native_field_elements(1)[0]
}

pub fn poseidon_config<F: PrimeField>() -> PoseidonConfig<F> {
    const RATE: usize = 2;
    const CAPACITY: usize = 1;
    const FULL_ROUNDS: usize = 8;
    const PARTIAL_ROUNDS: usize = 57;
    const ALPHA: u64 = 5;
    const SKIP_MATRICES: u64 = 0;

    let (ark, mds) = find_poseidon_ark_and_mds::<F>(
        F::MODULUS_BIT_SIZE as u64,
        RATE,
        FULL_ROUNDS as u64,
        PARTIAL_ROUNDS as u64,
        SKIP_MATRICES,
    );

    PoseidonConfig::new(
        FULL_ROUNDS,
        PARTIAL_ROUNDS,
        ALPHA,
        mds,
        ark,
        RATE,
        CAPACITY,
    )
}

pub fn fr_to_be_bytes(value: &Fr) -> [u8; 32] {
    let bytes = (*value).into_bigint().to_bytes_be();
    let mut out = [0u8; 32];
    let start = out.len().saturating_sub(bytes.len());
    out[start..].copy_from_slice(&bytes);
    out
}

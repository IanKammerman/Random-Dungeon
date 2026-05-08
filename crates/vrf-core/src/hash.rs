use ark_bn254::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use sha2::{Digest, Sha256};

pub fn alpha_to_fr(alpha: impl AsRef<[u8]>) -> Fr {
    let digest = Sha256::digest(alpha.as_ref());
    Fr::from_be_bytes_mod_order(&digest)
}

pub fn field_hash(x: Fr) -> Fr {
    // MVP SNARK-friendly algebraic hash placeholder. Replace with Poseidon before
    // using this construction as a production VRF.
    let y = x + Fr::from(5u64);
    let y2 = y.square();
    let y4 = y2.square();
    y4 * y + x * Fr::from(7u64) + Fr::from(42u64)
}

pub fn fr_to_be_bytes(value: &Fr) -> [u8; 32] {
    let bytes = (*value).into_bigint().to_bytes_be();
    let mut out = [0u8; 32];
    let start = out.len().saturating_sub(bytes.len());
    out[start..].copy_from_slice(&bytes);
    out
}

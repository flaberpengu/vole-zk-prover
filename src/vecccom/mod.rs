use std::{str::FromStr};
use rand::prelude::*;
use ff::{PrimeField, Field};
use lazy_static::lazy_static;
use num_bigint::BigUint;

use crate::{Fr, FrRepr};

lazy_static! {
    pub static ref FIELD_MODULUS: BigUint = BigUint::from_str("21888242871839275222246405745257275088548364400416034343698204186575808495617").unwrap();
    /// 2^64
    pub static ref TWO_64: Fr = Fr::from_str_vartime("18446744073709551616").unwrap();
    /// 2^128
    pub static ref TWO_128: Fr = Fr::from_str_vartime("340282366920938463463374607431768211456").unwrap();
    /// 2^192
    pub static ref TWO_192: Fr = Fr::from_str_vartime("6277101735386680763835789423207666416102355444464034512896").unwrap();
}
/// U128 representation of 21888242871839275222246405745257275088548364400416034343698204186575808495617
const MODULUS_AS_U64s: [u64; 4] = [
    0x30644E72E131A029,
    0xB85045B68181585D, 
    0x2833E84879B97091,
    0x43E1F593F0000001
];

/// Used for rejection sampling
pub fn u64s_overflow_field(x: &[u64; 4]) -> bool {
    (x[0] > MODULUS_AS_U64s[0])
 || (x[0] == MODULUS_AS_U64s[0] && x[1] > MODULUS_AS_U64s[1])
 || (x[0] == MODULUS_AS_U64s[0] && x[1] == MODULUS_AS_U64s[1] && x[2] > MODULUS_AS_U64s[2]) 
 || (x[0] == MODULUS_AS_U64s[0] && x[1] == MODULUS_AS_U64s[1] && x[2] == MODULUS_AS_U64s[2] && x[3] >= MODULUS_AS_U64s[3])
}

/// Efficient way of making an Fr from a big-endian u64 array representing that number
/// Does not check for overflow
pub fn unchecked_fr_from_be_u64_slice(from: &[u64; 4]) -> Fr {
    let mut repr = [0u8; 32];
    repr[0..8].copy_from_slice(&from[0].to_be_bytes());
    repr[8..16].copy_from_slice(&from[1].to_be_bytes());
    repr[16..24].copy_from_slice(&from[2].to_be_bytes());
    repr[24..32].copy_from_slice(&from[3].to_be_bytes());
    Fr::from_repr_vartime(FrRepr(repr)).unwrap()
}



/// Newer method much faster: use a CSPRNG
/// Returns N Frs
/// As long as the adversary doesn't learn the seed (for a couple reasons throughout the protocol, they shouldn't), they can't predict any of the outputs
pub fn expand_seed_to_Fr_vec(seed: [u8; 32], num_outputs: usize) -> Vec<Fr> {
    let seed: <StdRng as SeedableRng>::Seed = seed;
    let mut r = StdRng::from_seed(seed);
    let mut out: Vec<Fr> = Vec::with_capacity(num_outputs);


    for _i in 0..num_outputs {
        // Rejection sample a random 254 bit big-endian value
        // Generate 4 u64s and try to put them into a Fr
        // AND the two most significant bits with 00, producing a random 254 bit big-endian value
        
        let mut candidate = [
            r.next_u64() & 0x3F,
            r.next_u64(),
            r.next_u64(),
            r.next_u64(),
        ];

        while u64s_overflow_field(&candidate) {
            candidate = [
                r.next_u64() & 0x3F,
                r.next_u64(),
                r.next_u64(),
                r.next_u64(),
            ];
        }
        out.push(unchecked_fr_from_be_u64_slice(&candidate));
    }
    out
}

/// Instead of long vectors in most VOLE protocols, we're just doing a "vector" commitment to three values,
/// This means k for our SoftSpokenVOLE instantiation is 2, i.e. ∆ has just two bits of entropy.
/// Since we have to open and transmit all but one of the seeds, using a larger k for SoftSpokenVOLE doesn't save significant communication and solely wastes computation. 
pub fn commit_seeds(seed0: &Vec<u8>, seed1: &Vec<u8>) -> Vec<u8> {
    blake3::hash(
        &[
        *blake3::hash(seed0).as_bytes(),
        *blake3::hash(seed1).as_bytes(),
        ].concat()
    ).as_bytes().to_vec()
}

/// Just open one seed and hide the other since only two were committed :P. The proof an element is just the hash of the other hidden element
pub fn proof_for_revealed_seed(other_seed: &Vec<u8>) -> [u8; 32] {
    *blake3::hash(other_seed).as_bytes()
}

/// Verifies a proof for a committed seed
pub fn verify_proof_of_revealed_seed(
    commitment: &Vec<u8>,
    revealed_seed: &Vec<u8>, 
    revealed_seed_idx: bool, 
    proof: &[u8; 32]
) -> bool {
    let digest_of_revealed = *blake3::hash(revealed_seed).as_bytes();
    let preimage = 
    if revealed_seed_idx {
        [proof.clone(), digest_of_revealed].concat()
    } else {
        [digest_of_revealed, proof.clone()].concat()
    };
    blake3::hash(&preimage).as_bytes().to_vec() == *commitment
}

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test_seed_expansion_len() {
        let seed = [0u8; 32];
        assert_eq!(super::expand_seed_to_Fr_vec(seed.clone(), 1).len(), 1);
        assert_eq!(super::expand_seed_to_Fr_vec(seed.clone(), 2).len(), 2);
        assert_eq!(super::expand_seed_to_Fr_vec(seed.clone(), 4).len(), 4);
        assert_eq!(super::expand_seed_to_Fr_vec(seed.clone(), 1000).len(), 1000);
    }

    #[test]
    // TODO: add more edge cases
    fn test_fr_from_512bits() {
        // Try with modulus + 1
        let mut x = [0u64; 4];
        x[0] = 0x30644E72E131A029;
        x[1] = 0xB85045B68181585D;
        x[2] = 0x2833E84879B97091;
        x[3] = 0x43E1F593F0000002;

        assert_eq!(unchecked_fr_from_be_u64_slice(&x), Fr::from_str_vartime("1").unwrap());

        x[0] = 0x30644E72E131A029;
        x[1] = 0xB85045B68181585D;
        x[2] = 0x2833E84879B97091;
        x[3] = 0x43E1F593F0000001;

        assert_eq!(unchecked_fr_from_be_u64_slice(&x), Fr::from_str_vartime("0").unwrap());

        x[0] = 0x0;
        x[1] = 0x0;
        x[2] = 0x0;
        x[3] = 0x03;

        assert_eq!(unchecked_fr_from_be_u64_slice(&x), Fr::from_str_vartime("3").unwrap());
    }
    
    #[test]
    fn test_seed_commit_prove() {
        let seed0 = [5u8; 32].to_vec();
        let seed1 = [6u8; 32].to_vec();
        let commitment = commit_seeds(&seed0, &seed1);

        let proof0 = proof_for_revealed_seed(&seed1);
        let proof1 = proof_for_revealed_seed(&seed0);

        assert!(verify_proof_of_revealed_seed(&commitment, &seed0, false, &proof0));
        assert!(!verify_proof_of_revealed_seed(&commitment, &seed0, true, &proof0));

        assert!(verify_proof_of_revealed_seed(&commitment, &seed1, true, &proof1));
        assert!(!verify_proof_of_revealed_seed(&commitment, &seed1, false, &proof1));

        assert!(!verify_proof_of_revealed_seed(&commitment, &seed0, true, &proof1));
        assert!(!verify_proof_of_revealed_seed(&commitment, &seed0, false, &proof1));

    }
}
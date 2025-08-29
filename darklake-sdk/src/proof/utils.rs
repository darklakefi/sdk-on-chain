use ark_bn254_4::Fr;
use num_bigint::BigUint;
use poseidon_ark as poseidon;
use std::str::FromStr;
use password_hash::rand_core::{OsRng, RngCore};

pub fn u64_array_to_u8_array_le(input: &[u64; 4]) -> [u8; 32] {
    let mut output = [0u8; 32];
    for (i, &val) in input.iter().enumerate() {
        // Convert each u64 to its little-endian byte representation
        let bytes = val.to_le_bytes();
        // Copy the 8 bytes into the correct slice of the output array
        output[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
    }
    output
}

pub fn compute_poseidon_hash_with_salt(min_out: u64, salt: [u8; 8]) -> [u64; 4] {
    let pos = poseidon::Poseidon::new();

    let min_out_field_element = Fr::from(min_out);

    let salt_u64 = u64::from_le_bytes(salt);
    let salt_field_element = Fr::from(salt_u64);

    let inputs_for_poseidon: Vec<Fr> = vec![min_out_field_element, salt_field_element];

    let hash_output_bytes = pos.hash(inputs_for_poseidon).unwrap(); // Handle the Result

    let hash = hash_output_bytes.0.0;

    hash
}

pub fn compute_poseidon_hash(min_out: u64) -> [u64; 4] {
    let mut rng = OsRng;

    let mut raw_salt_bytes_8 = [0u8; 8];
    rng.fill_bytes(&mut raw_salt_bytes_8); // Fill with 32 cryptographically random bytes

    let hash = compute_poseidon_hash_with_salt(min_out, raw_salt_bytes_8);

    hash
}

pub fn bytes_to_bigint(bytes: &[u8; 32]) -> BigUint {
    // Step 1: Convert bytes to BigInt (little-endian) - equivalent to F.fromRprLE(o)
    let value = BigUint::from_bytes_le(bytes);

    // Step 2: Apply Montgomery conversion (equivalent to JavaScript F.fromRprLEM())
    // For BN254 field, p = 21888242871839275222246405745257275088548364400416034343698204186575808495617
    let p = BigUint::from_str(
        "21888242871839275222246405745257275088548364400416034343698204186575808495617",
    )
    .unwrap();

    // Ri = R^(-1) mod p (Montgomery inverse)
    // For BN254, Ri = 9915499612839321149637521777990102151350674507940716049588462388200839649614
    let ri = BigUint::from_str(
        "9915499612839321149637521777990102151350674507940716049588462388200839649614",
    )
    .unwrap();

    // Apply Montgomery conversion: value * Ri mod p
    let result = (value * ri) % &p;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_poseidon_hash() {
        // Test with a small value
        let min_out = 1000u64;
        compute_poseidon_hash(min_out);

        // Test with zero
        compute_poseidon_hash(0u64);

        // Test with a large value
        compute_poseidon_hash(u64::MAX);

        // Test with a medium value
        compute_poseidon_hash(123456789u64);
    }

    #[test]
    fn test_poseidon_hash_consistency() {
        // Test that the same input produces consistent output
        let min_out = 12345u64;

        // Call compute_poseidon_hash multiple times with the same input
        // Note: This function generates random salt each time, so outputs will differ
        // But the function should not panic and should complete successfully
        for _ in 0..5 {
            compute_poseidon_hash(min_out);
        }
    }


    #[test]
    fn test_poseidon_hash_bytes_and_field_element_all_zeroes() {
        let min_out = 0u64;
        let salt = [0u8; 8];
        let hash = compute_poseidon_hash_with_salt(min_out, salt);
        let bytes = u64_array_to_u8_array_le(&hash);

        let expected_bytes = [
            130, 154, 1, 250, 228, 248, 226, 43, 27, 76, 165, 173, 91, 84, 165, 131, 78, 224, 152,
            167, 123, 115, 91, 213, 116, 49, 167, 101, 109, 41, 161, 8,
        ];

        assert_eq!(bytes, expected_bytes, "Hash bytes should match expected");

        // Convert to field element to scalar
        let field_element = bytes_to_bigint(&bytes);
        let field_element_str = field_element.to_string();
        let expected_value =
            "14744269619966411208579211824598458697587494354926760081771325075741142829156";
        assert_eq!(
            field_element_str, expected_value,
            "Field element should match expected"
        );
    }

    #[test]
    fn test_poseidon_hash_bytes_and_field_element_non_zero_min_out() {
        let min_out = 1u64;
        let salt = [0u8; 8];
        let hash = compute_poseidon_hash_with_salt(min_out, salt);
        let bytes = u64_array_to_u8_array_le(&hash);

        let expected_bytes = [
            153, 228, 180, 254, 17, 76, 70, 85, 144, 220, 166, 91, 235, 153, 101, 2, 209, 78, 60,
            87, 166, 84, 127, 81, 221, 96, 78, 137, 198, 139, 168, 47,
        ];

        assert_eq!(bytes, expected_bytes, "Hash bytes should match expected");

        // Convert to field element to scalar
        let field_element = bytes_to_bigint(&bytes);
        let field_element_str = field_element.to_string();
        let expected_value =
            "18423194802802147121294641945063302532319431080857859605204660473644265519999";
        assert_eq!(
            field_element_str, expected_value,
            "Field element should match expected"
        );
    }

    #[test]
    fn test_poseidon_hash_bytes_and_field_element_non_zero_salt() {
        let min_out = 0u64;
        let mut salt = [0u8; 8];
        salt[0] = 100;
        let hash = compute_poseidon_hash_with_salt(min_out, salt);
        let bytes = u64_array_to_u8_array_le(&hash);

        let expected_bytes = [
            1, 81, 179, 227, 61, 198, 154, 248, 208, 143, 160, 176, 87, 254, 14, 196, 209, 124,
            218, 27, 125, 233, 182, 32, 41, 138, 181, 91, 71, 156, 157, 9,
        ];

        assert_eq!(bytes, expected_bytes, "Hash bytes should match expected");

        // Convert to field element to scalar
        let field_element = bytes_to_bigint(&bytes);
        let field_element_str = field_element.to_string();
        let expected_value =
            "8495383626315836305837861875604061881947184042460352587383381292552921449";
        assert_eq!(
            field_element_str, expected_value,
            "Field element should match expected"
        );
    }

    #[test]
    fn test_bytes_to_bigint() {
        // Test with the expected bytes directly (from the existing test)
        let expected_bytes = [
            130, 154, 1, 250, 228, 248, 226, 43, 27, 76, 165, 173, 91, 84, 165, 131, 78, 224, 152,
            167, 123, 115, 91, 213, 116, 49, 167, 101, 109, 41, 161, 8,
        ];

        let bigint_result = bytes_to_bigint(&expected_bytes);
        let bigint_str = bigint_result.to_string();

        let expected_value =
            "14744269619966411208579211824598458697587494354926760081771325075741142829156";

        assert_eq!(
            bigint_str, expected_value,
            "Ones bytes should convert correctly"
        );
    }
}
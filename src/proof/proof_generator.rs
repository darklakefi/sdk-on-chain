use anyhow::Result;
use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_circom::{CircomBuilder, CircomConfig, CircomReduction};
use ark_ec::AffineRepr;
use ark_ff::{BigInt, PrimeField};
use ark_groth16::{Groth16, Proof};
use ark_std::rand::thread_rng;
use num_bigint::BigUint;
use num_traits::Num;

use std::ops::Neg;
use std::path::Path;
type GrothBn = Groth16<Bn254, CircomReduction>;

/// Represents the inputs for proof generation
#[derive(Debug, Clone)]
pub struct PrivateProofInputs {
    pub min_out: u64,
    pub salt: u64,
}

#[derive(Debug, Clone)]
pub struct PublicProofInputs {
    pub real_out: u64,
    pub commitment: BigUint,
}

/// Represents the generated proof components
#[derive(Debug, Clone)]
pub struct GeneratedProof {
    pub proof_a: [u8; 64],
    pub proof_b: [u8; 128],
    pub proof_c: [u8; 64],
    pub public_signals: Vec<[u8; 32]>,
}

/// Converts an ark_ff::BigInt to a 32-byte big-endian byte array
fn bigint_to_bytes_be(bigint: &BigInt<4>) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    // Write each limb (u64) into the buffer in little-endian order
    for (i, limb) in bigint.0.iter().enumerate() {
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    // The buffer is now little-endian; reverse for big-endian
    bytes.reverse();
    bytes
}

/// Finds the correct path to circuit files regardless of where the code is executed from
pub fn find_circuit_path(filename: &str) -> String {
    // Simply use CARGO_MANIFEST_DIR which points to the darklake-sdk directory
    // and construct the path to circuits from there
    let circuits_path = format!(
        "{}/src/proof/circuits/{}",
        env!("CARGO_MANIFEST_DIR"),
        filename
    );
    circuits_path
}

/// Generates a Groth16 proof using WASM circuits
///
/// # Arguments
/// * `private_inputs` - The private inputs (minOut, salt)
/// * `public_inputs` - The public inputs (realOut, commitment)
/// * `is_cancel` - Whether this is a cancel proof (false for settle)
///
/// # Returns
/// * `Result<GeneratedProof>` - The generated proof components
pub fn generate_proof(
    private_inputs: &PrivateProofInputs,
    public_inputs: &PublicProofInputs,
    is_cancel: bool,
) -> Result<(Proof<Bn254>, Vec<Fr>)> {
    let file_prefix = if is_cancel { "cancel" } else { "settle" };

    // Construct paths to WASM and zkey files using dynamic path resolution
    let wasm_path = find_circuit_path(&format!("{}.wasm", file_prefix));
    let zkey_path = find_circuit_path(&format!("{}_final.zkey", file_prefix));
    let r1cs_path = find_circuit_path(&format!("{}.r1cs", file_prefix));

    // Check if files exist
    if !Path::new(&wasm_path).exists() {
        return Err(anyhow::anyhow!("WASM file not found: {}", wasm_path));
    }
    if !Path::new(&zkey_path).exists() {
        return Err(anyhow::anyhow!("ZKey file not found: {}", zkey_path));
    }
    if !Path::new(&r1cs_path).exists() {
        return Err(anyhow::anyhow!("R1CS file not found: {}", r1cs_path));
    }

    // // Create Circom configuration
    let cfg = CircomConfig::<Fr>::new(&wasm_path, &r1cs_path).unwrap();

    // // Build the circuit
    let mut builder = CircomBuilder::new(cfg);

    // // Add private inputs
    builder.push_input("minOut", private_inputs.min_out);
    builder.push_input("salt", private_inputs.salt);

    // // Add public inputs
    builder.push_input("realOut", public_inputs.real_out);

    // Convert commitment byte array to BigUint
    builder.push_input("commitment", public_inputs.commitment.clone());
    let mut rng = thread_rng();

    let mut key_file = std::fs::File::open(zkey_path).unwrap();
    let (params, _) = ark_circom::read_zkey(&mut key_file).unwrap();

    // // Generate the proof
    let circom = builder.build().unwrap();

    let public_inputs = circom.get_public_inputs().unwrap();

    // inputs returned mainly for ease of testing (needs arkworks type)
    return Ok((
        GrothBn::create_random_proof_with_reduction(circom, &params, &mut rng).unwrap(),
        public_inputs,
    ));
}

// default rust proof needs adjusting to match the solana proof format
pub fn convert_proof_to_solana_proof(
    proof: &Proof<Bn254>,
    public_inputs: &PublicProofInputs,
) -> GeneratedProof {
    // Convert proof components to byte arrays (equivalent to JavaScript conversion)
    let proof_a = negate_and_serialize_g1(&proof.a);
    let proof_b = g2_uncompressed(&proof.b);
    let proof_c = g1_uncompressed(&proof.c);

    // Convert public signals to 32-byte buffers (equivalent to JavaScript to32ByteBuffer)
    // For now, we'll use the public inputs that were passed to the function
    // In a real implementation, these would come from the proof's public inputs
    let public_signals: Vec<[u8; 32]> = vec![
        to_32_byte_buffer(&BigUint::from(public_inputs.real_out)),
        to_32_byte_buffer(&public_inputs.commitment),
    ];

    GeneratedProof {
        proof_a,
        proof_b,
        proof_c,
        public_signals,
    }
}

/// Converts a BigUint to a 32-byte buffer (equivalent to JavaScript to32ByteBuffer)
pub fn to_32_byte_buffer(big_int: &BigUint) -> [u8; 32] {
    let mut buffer = [0u8; 32];
    let hex_string = format!("{:064x}", big_int); // Pad to 64 hex chars (32 bytes)
    let bytes = hex::decode(&hex_string).unwrap();
    buffer.copy_from_slice(&bytes);
    buffer
}

/// Converts a 32-byte buffer back to a BigUint (reverse of to_32_byte_buffer)
pub fn from_32_byte_buffer(buffer: &[u8; 32]) -> BigUint {
    let hex_string = hex::encode(buffer);
    BigUint::from_str_radix(&hex_string, 16).unwrap()
}

/// Converts a G1 point to uncompressed 64-byte format (equivalent to JavaScript g1Uncompressed)
fn g1_uncompressed(point: &G1Affine) -> [u8; 64] {
    let mut out = [0u8; 64];

    let x_bigint = point.x().unwrap().into_bigint();
    let y_bigint = point.y().unwrap().into_bigint();

    // Convert x and y coordinates to big-endian bytes
    let x_bytes = bigint_to_bytes_be(&x_bigint);
    let y_bytes = bigint_to_bytes_be(&y_bigint);

    // Copy x and y bytes to output buffer
    out[0..32].copy_from_slice(&x_bytes);
    out[32..64].copy_from_slice(&y_bytes);

    out
}

/// Negates a G1 point and serializes it (equivalent to JavaScript negateAndSerializeG1)
fn negate_and_serialize_g1(point: &G1Affine) -> [u8; 64] {
    let negated = point.neg();
    g1_uncompressed(&negated)
}

/// Converts a G2 point to uncompressed 128-byte format (equivalent to JavaScript g2Uncompressed)
fn g2_uncompressed(point: &G2Affine) -> [u8; 128] {
    let mut out = [0u8; 128];

    // G2 points have coordinates in Fq2 (quadratic extension)
    // Each coordinate is (x0 + x1*u, y0 + y1*u) where each component is 32 bytes (4 x 32 bytes)
    let x_coord = point.x().unwrap();
    let y_coord = point.y().unwrap();

    // Extract the Fq components from the quadratic extension
    let x0_bigint = x_coord.c0.into_bigint();
    let x1_bigint = x_coord.c1.into_bigint();
    let y0_bigint = y_coord.c0.into_bigint();
    let y1_bigint = y_coord.c1.into_bigint();

    // Convert each component to big-endian bytes
    let x0_bytes = bigint_to_bytes_be(&x0_bigint);
    let x1_bytes = bigint_to_bytes_be(&x1_bigint);
    let y0_bytes = bigint_to_bytes_be(&y0_bigint);
    let y1_bytes = bigint_to_bytes_be(&y1_bigint);

    // Layout: [x1 (32 bytes), x0 (32 bytes), y1 (32 bytes), y0 (32 bytes)]
    out[0..32].copy_from_slice(&x1_bytes);
    out[32..64].copy_from_slice(&x0_bytes);
    out[64..96].copy_from_slice(&y1_bytes);
    out[96..128].copy_from_slice(&y0_bytes);

    out
}

pub fn create_circom_config(
    wasm_path: &str,
    r1cs_path: &str,
) -> Result<CircomBuilder<ark_ff::Fp<ark_ff::MontBackend<ark_bn254::FrConfig, 4>, 4>>> {
    let config = CircomConfig::new(wasm_path, r1cs_path)
        .map_err(|e| anyhow::anyhow!("Failed to create circom config: {}", e))?;
    let builder = CircomBuilder::new(config);
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ark_circom::read_zkey;
    use ark_ff::{BigInt, PrimeField};
    use ark_groth16::prepare_verifying_key;

    type GrothBn = Groth16<Bn254, CircomReduction>;

    use crate::proof::utils::{
        bytes_to_bigint, compute_poseidon_hash_with_salt, u64_array_to_u8_array_le,
    };

    use super::*;

    #[test]
    fn test_bigint_to_bytes_be() {
        // Test with a simple BigInt value
        let bigint = BigInt::from(12345u64);
        let bytes = bigint_to_bytes_be(&bigint);

        // The first 24 bytes should be zeros, and the last 8 bytes should represent 12345
        assert_eq!(bytes[0..24], [0u8; 24]);
        assert_eq!(bytes[24..32], [0, 0, 0, 0, 0, 0, 48, 57]); // 12345 in big-endian

        println!("✓ Test passed for BigInt to bytes conversion");
    }

    #[test]
    fn test_to_32_byte_buffer_and_from_32_byte_buffer_are_reverse() {
        // Test with various BigUint values
        let test_cases = vec![
            BigUint::from(0u64),
            BigUint::from(1u64),
            BigUint::from(12345u64),
            BigUint::from(u64::MAX),
            BigUint::from_str_radix("123456789012345678901234567890", 10).unwrap(),
            BigUint::from_str_radix(
                "21888242871839275222246405745257275088548364400416034343698204186575808495617",
                10,
            )
            .unwrap(),
        ];

        for original in test_cases {
            // Convert to 32-byte buffer
            let buffer = to_32_byte_buffer(&original);

            // Convert back to BigUint
            let reconstructed = from_32_byte_buffer(&buffer);

            // Verify they are equal
            assert_eq!(
                original, reconstructed,
                "Failed for value: {}. Original: {}, Reconstructed: {}",
                original, original, reconstructed
            );

            println!("✓ Test passed for value: {}", original);
        }
    }

    #[tokio::test]
    async fn test_settle_proof_generation_verification() {
        let salt_bytes = [0; 8];
        let commitment = bytes_to_bigint(&u64_array_to_u8_array_le(
            &compute_poseidon_hash_with_salt(1, salt_bytes),
        ));

        let private_inputs = PrivateProofInputs {
            min_out: 1,
            salt: 0,
        };

        let public_inputs = PublicProofInputs {
            real_out: 181404864,
            commitment,
        };

        // This test will fail without actual circuit files, but it demonstrates the API
        let result = generate_proof(&private_inputs, &public_inputs, false);

        let zkey_path = find_circuit_path("settle_final.zkey");

        let mut key_file = std::fs::File::open(zkey_path).unwrap();
        let (params, _) = read_zkey(&mut key_file).unwrap();

        let (proof, public_inputs) = result.unwrap();

        let pvk = prepare_verifying_key(&params.vk);

        let verified = GrothBn::verify_proof(&pvk, &proof, &public_inputs).unwrap();

        // For now, we expect this to fail since we don't have the actual circuit files
        assert!(verified);
    }

    #[tokio::test]
    async fn test_cancel_proof_generation_verification() {
        let salt_bytes = [0; 8];
        let commitment = bytes_to_bigint(&u64_array_to_u8_array_le(
            &compute_poseidon_hash_with_salt(10_000_000_000, salt_bytes),
        ));

        let private_inputs = PrivateProofInputs {
            min_out: 10_000_000_000, // has to violate the real out
            salt: 0,
        };

        let public_inputs = PublicProofInputs {
            real_out: 181_404_864,
            commitment,
        };

        // This test will fail without actual circuit files, but it demonstrates the API
        let result = generate_proof(&private_inputs, &public_inputs, true);

        let zkey_path = find_circuit_path("cancel_final.zkey");

        let mut key_file = std::fs::File::open(zkey_path).unwrap();
        let (params, _) = read_zkey(&mut key_file).unwrap();

        let (proof, public_inputs) = result.unwrap();

        let pvk = prepare_verifying_key(&params.vk);

        let verified = GrothBn::verify_proof(&pvk, &proof, &public_inputs).unwrap();

        // For now, we expect this to fail since we don't have the actual circuit files
        assert!(verified);
    }

    #[tokio::test]
    async fn test_solana_proof_conversion() {
        let salt_bytes = [0; 8];
        let commitment = bytes_to_bigint(&u64_array_to_u8_array_le(
            &compute_poseidon_hash_with_salt(0, salt_bytes),
        ));

        // Leaving here to know what params were used to generate the proof
        // let private_inputs = PrivateProofInputs {
        //     min_out: 0,
        //     salt: 0,
        // };

        let public_inputs = PublicProofInputs {
            real_out: 181404864,
            commitment,
        };

        // Create the proof components manually
        // For G1Affine, we need to use Fq (base field) for coordinates, not Fr (scalar field)
        let a = G1Affine::new(
            ark_bn254::Fq::from_bigint(
                BigInt::from_str(
                    "21336970266497842767908716900339070049495537133979236291184473560261457202592",
                )
                .unwrap(),
            )
            .unwrap(),
            ark_bn254::Fq::from_bigint(
                BigInt::from_str(
                    "10357002110641456830517929333644944387172532981420682841938017904947253585119",
                )
                .unwrap(),
            )
            .unwrap(),
        );

        // For G2Affine, we need to create the quadratic extension field
        // The b component has two parts: (x0 + x1*u, y0 + y1*u)
        let b_x0 = ark_bn254::Fq::from_bigint(
            BigInt::from_str(
                "10031611684880255122878343088918815315994982761231989012341296266256938728077",
            )
            .unwrap(),
        )
        .unwrap();
        let b_x1 = ark_bn254::Fq::from_bigint(
            BigInt::from_str(
                "9864564115022077789419921627032874448498818722354799099437941413808450142573",
            )
            .unwrap(),
        )
        .unwrap();
        let b_y0 = ark_bn254::Fq::from_bigint(
            BigInt::from_str(
                "3480829982942279746431969300864501854801665728182328337962114289772946080951",
            )
            .unwrap(),
        )
        .unwrap();
        let b_y1 = ark_bn254::Fq::from_bigint(
            BigInt::from_str(
                "16235198704867435558683146462392128588420589574937794617432543654822315372734",
            )
            .unwrap(),
        )
        .unwrap();

        // Create the quadratic extension field elements
        let b_x = ark_bn254::Fq2::new(b_x0, b_x1);
        let b_y = ark_bn254::Fq2::new(b_y0, b_y1);
        let b = G2Affine::new(b_x, b_y);

        let c = G1Affine::new(
            ark_bn254::Fq::from_bigint(
                BigInt::from_str(
                    "18355236746471312442545116245056291713255790355048937153933495531313019379566",
                )
                .unwrap(),
            )
            .unwrap(),
            ark_bn254::Fq::from_bigint(
                BigInt::from_str(
                    "18641767980306819609170870710163640532602332263544376747995786075293749543061",
                )
                .unwrap(),
            )
            .unwrap(),
        );

        // Create the complete proof
        let proof = Proof { a, b, c };

        let solana_proof = convert_proof_to_solana_proof(&proof, &public_inputs);

        // Expected proof components from the provided test data
        let expected_proof_a: [u8; 64] = [
            47, 44, 76, 21, 126, 202, 243, 46, 235, 135, 162, 57, 164, 197, 50, 168, 136, 199, 95,
            241, 187, 183, 172, 191, 78, 29, 130, 177, 191, 165, 205, 160, 25, 126, 115, 115, 144,
            158, 228, 128, 166, 92, 240, 24, 9, 92, 227, 174, 4, 13, 224, 148, 145, 54, 29, 196,
            70, 201, 135, 171, 249, 175, 88, 104,
        ];

        let expected_proof_b: [u8; 128] = [
            21, 207, 37, 58, 228, 149, 76, 73, 42, 190, 131, 167, 8, 96, 65, 141, 60, 42, 155, 231,
            62, 53, 119, 167, 164, 75, 174, 175, 155, 229, 129, 109, 22, 45, 176, 229, 160, 239,
            161, 172, 251, 6, 183, 217, 223, 47, 61, 31, 164, 95, 104, 80, 187, 77, 120, 176, 210,
            23, 149, 124, 72, 120, 122, 141, 35, 228, 203, 252, 239, 240, 195, 63, 59, 152, 45,
            248, 188, 82, 131, 158, 199, 96, 154, 166, 161, 115, 166, 70, 103, 149, 25, 174, 62,
            233, 48, 190, 7, 178, 20, 119, 210, 128, 22, 106, 28, 244, 39, 249, 64, 232, 192, 108,
            234, 154, 115, 28, 39, 187, 185, 74, 211, 132, 153, 121, 180, 81, 68, 183,
        ];

        let expected_proof_c: [u8; 64] = [
            40, 148, 178, 34, 95, 147, 249, 78, 211, 28, 202, 71, 162, 82, 131, 150, 213, 244, 32,
            183, 29, 233, 83, 95, 38, 163, 180, 110, 53, 174, 95, 110, 41, 54, 221, 228, 133, 189,
            61, 191, 111, 39, 198, 81, 208, 14, 69, 84, 114, 25, 92, 155, 135, 247, 155, 114, 173,
            228, 136, 166, 9, 51, 0, 149,
        ];

        // Verify that the generated proof components match the expected values
        assert_eq!(
            solana_proof.proof_a, expected_proof_a,
            "Proof A does not match expected value"
        );
        assert_eq!(
            solana_proof.proof_b, expected_proof_b,
            "Proof B does not match expected value"
        );
        assert_eq!(
            solana_proof.proof_c, expected_proof_c,
            "Proof C does not match expected value"
        );

        println!("✓ All proof components match expected values");
    }
}

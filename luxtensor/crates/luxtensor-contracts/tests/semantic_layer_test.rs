use luxtensor_contracts::ai_precompiles::{
    AIPrecompileState, vector_store_precompile, vector_query_precompile, is_semantic_precompile
};
use revm::primitives::Bytes;

#[test]
fn test_semantic_layer_integration() {
    let state = AIPrecompileState::new();
    let gas_limit = 1_000_000;

    // 1. Create a test vector
    let vector_id: u64 = 42;
    // Default dimension is 768
    let vector_data: Vec<f32> = vec![0.5; 768];
    let vector_len = vector_data.len() as u32;

    // Encode input for VECTOR_STORE (0x20)
    // Format: [ID: 32 bytes] [Offset: 32 bytes] [Length: 32 bytes] [Data...]
    let mut store_input = Vec::new();

    // ID (padded to 32 bytes)
    store_input.extend_from_slice(&[0u8; 24]);
    store_input.extend_from_slice(&vector_id.to_be_bytes());

    // Offset (96 bytes - fixed for this test)
    store_input.extend_from_slice(&[0u8; 31]);
    store_input.push(96);

    // Length (32 bytes)
    store_input.extend_from_slice(&[0u8; 28]);
    store_input.extend_from_slice(&vector_len.to_be_bytes());

    // Data (floats encoded as u32 big-endian)
    for float_val in &vector_data {
        let bits = float_val.to_bits();
        store_input.extend_from_slice(&bits.to_be_bytes());
    }

    // Call VECTOR_STORE
    let result = vector_store_precompile(
        &Bytes::from(store_input),
        gas_limit,
        &state
    );

    assert!(result.is_ok(), "Vector store failed");
    let output = result.unwrap();
    assert_eq!(output.bytes[31], 1, "Vector store should return true");

    // 2. Query the vector
    // Format: [K: 32 bytes] [Offset: 32 bytes] [Length: 32 bytes] [Data...]
    let k: u64 = 5;
    let mut query_input = Vec::new();

    // K
    query_input.extend_from_slice(&[0u8; 24]);
    query_input.extend_from_slice(&k.to_be_bytes());

    // Offset
    query_input.extend_from_slice(&[0u8; 31]);
    query_input.push(96);

    // Length
    query_input.extend_from_slice(&[0u8; 28]);
    query_input.extend_from_slice(&vector_len.to_be_bytes());

    // Query with same vector to get top match
    for float_val in &vector_data {
        let bits = float_val.to_bits();
        query_input.extend_from_slice(&bits.to_be_bytes());
    }

    // Call VECTOR_QUERY
    let result = vector_query_precompile(
        &Bytes::from(query_input),
        gas_limit,
        &state
    );

    assert!(result.is_ok(), "Vector query failed");
    let output = result.unwrap();

    // Parse output - new format with both IDs and Scores
    // Format: [Offset IDs: 32] [Offset Scores: 32] [Len IDs: 32] [IDs...] [Len Scores: 32] [Scores...]

    // First word: Offset to IDs array (64)
    assert_eq!(output.bytes[31], 64, "First offset should be 64");

    // Second word: Offset to Scores array (varies based on result count)
    // Skip checking specific value as it depends on result count

    // Third word (at byte 64): Length of IDs array
    let res_len = u64::from_be_bytes(output.bytes[88..96].try_into().unwrap());
    assert!(res_len >= 1, "Should find at least 1 result");

    // Fourth word (at byte 96): First ID
    let found_id = u64::from_be_bytes(output.bytes[120..128].try_into().unwrap());
    assert_eq!(found_id, vector_id, "Should find the stored vector ID");
}

#[test]
fn test_semantic_address_check() {
    let mut addr = [0u8; 20];
    addr[19] = 0x20;
    assert!(is_semantic_precompile(&addr));

    addr[19] = 0x21;
    assert!(is_semantic_precompile(&addr));

    addr[19] = 0x22;
    assert!(!is_semantic_precompile(&addr));
}

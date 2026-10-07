use ace_core::{AceError, AceResult};

/// Returns a verbatim copy of the source bytes.
pub fn raw_encode(input: &[u8]) -> Vec<u8> {
    input.to_vec()
}

/// Validates and reconstructs a RAW payload.
pub fn raw_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if input.len() != expected_size {
        return Err(AceError::Malformed(
            "RAW payload length differs from original size",
        ));
    }
    Ok(input.to_vec())
}

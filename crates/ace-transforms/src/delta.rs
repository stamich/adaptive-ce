//! Byte-wise wrapping delta transform (`out[i] = in[i] - in[i-1]`).

/// Encodes bytes as wrapping differences from the preceding source byte.
pub fn delta_encode(input: &[u8]) -> Vec<u8> {
    let mut previous = 0u8;
    input
        .iter()
        .map(|&byte| {
            let delta = byte.wrapping_sub(previous);
            previous = byte;
            delta
        })
        .collect()
}

/// Reconstructs bytes previously encoded by [`delta_encode`] (running wrapping sum).
pub fn delta_decode(input: &[u8]) -> Vec<u8> {
    let mut current = 0u8;
    input
        .iter()
        .map(|&delta| {
            current = current.wrapping_add(delta);
            current
        })
        .collect()
}

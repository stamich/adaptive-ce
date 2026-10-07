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

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact round-trip including wrapping transitions; the first byte is stored verbatim.
    #[test]
    fn delta_round_trip() {
        let data = [254, 255, 0, 1, 200, 12];
        let encoded = delta_encode(&data);
        assert_eq!(encoded[..2], [254, 1]);
        assert_eq!(delta_decode(&encoded), data);
        assert!(delta_encode(&[]).is_empty());
    }
}

//! Reversible preprocessing transforms for ACE 0.2.1.

use ace_core::{AceResult, TransformId};

/// Applies one forward preprocessing transform.
pub fn apply_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_encode(input)),
    }
}

/// Applies one inverse preprocessing transform.
pub fn invert_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_decode(input)),
    }
}

/// Encodes bytes as wrapping differences from the preceding source byte.
pub fn delta_encode(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let mut out = Vec::with_capacity(input.len());
    out.push(input[0]);
    for pair in input.windows(2) { out.push(pair[1].wrapping_sub(pair[0])); }
    out
}

/// Reconstructs bytes previously encoded by [`delta_encode`].
pub fn delta_decode(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let mut out = Vec::with_capacity(input.len());
    let mut current = input[0];
    out.push(current);
    for &delta in &input[1..] {
        current = current.wrapping_add(delta);
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies exact round-trip including wrapping byte transitions.
    #[test]
    fn delta_round_trip() {
        let data = [254, 255, 0, 1, 200, 12];
        assert_eq!(delta_decode(&delta_encode(&data)), data);
    }
}

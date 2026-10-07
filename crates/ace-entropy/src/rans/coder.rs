//! Scalar 32-bit rANS encoder and decoder (byte alphabet, 12-bit probability scale).

use ace_core::{AceError, AceResult};

use super::model::{NormalizedFrequencyTable, RANS_L, RANS_SCALE, RANS_SCALE_BITS};

/// Encodes bytes using a scalar 32-bit range Asymmetric Numeral System.
pub fn rans_encode(input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    if input.is_empty() {
        return Ok((vec![0u8; 512], Vec::new()));
    }
    let model = NormalizedFrequencyTable::from_input(input)?;
    let mut state = RANS_L;
    let mut renorm = Vec::new();
    for &symbol in input.iter().rev() {
        let frequency = model.frequencies[symbol as usize] as u32;
        let start = model.cumulative[symbol as usize] as u32;
        if frequency == 0 {
            return Err(AceError::InvalidRans("input symbol has zero frequency"));
        }
        let x_max = ((RANS_L >> RANS_SCALE_BITS) << 8).saturating_mul(frequency);
        while state >= x_max {
            renorm.push((state & 0xff) as u8);
            state >>= 8;
        }
        state = ((state / frequency) << RANS_SCALE_BITS) + (state % frequency) + start;
    }
    let mut payload = Vec::with_capacity(4 + renorm.len());
    payload.extend_from_slice(&state.to_le_bytes());
    payload.extend(renorm.iter().rev().copied());
    Ok((model.encode_metadata(), payload))
}

/// Decodes a scalar rANS payload into exactly `expected_size` bytes.
pub fn rans_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    if input.len() < 4 {
        return Err(AceError::InvalidRans("truncated initial state"));
    }
    let model = NormalizedFrequencyTable::decode_metadata(metadata)?;
    if model.cumulative[256] as u32 != RANS_SCALE {
        return Err(AceError::InvalidRans("empty model for non-empty output"));
    }
    let mut state = u32::from_le_bytes(
        input[0..4]
            .try_into()
            .map_err(|_| AceError::InvalidRans("truncated state"))?,
    );
    if state < RANS_L {
        return Err(AceError::InvalidRans(
            "initial state below normalization bound",
        ));
    }
    let mut cursor = 4usize;
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    // Every slot in [0, RANS_SCALE) is covered because the model sums exactly to RANS_SCALE.
    let lookup = model.slot_lookup_table();
    while out.len() < expected_size {
        let slot = state & (RANS_SCALE - 1);
        let symbol = lookup[slot as usize];
        out.push(symbol);
        let frequency = model.frequencies[symbol as usize] as u32;
        let start = model.cumulative[symbol as usize] as u32;
        state = frequency
            .saturating_mul(state >> RANS_SCALE_BITS)
            .saturating_add(slot - start);
        while state < RANS_L {
            let next = *input
                .get(cursor)
                .ok_or(AceError::InvalidRans("truncated renormalization bytes"))?
                as u32;
            cursor += 1;
            state = (state << 8) | next;
        }
    }
    if cursor != input.len() {
        return Err(AceError::InvalidRans("trailing rANS payload bytes"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies exact rANS round-trip for skewed and full-alphabet inputs.
    #[test]
    fn round_trip() {
        let mut data = vec![b'a'; 20_000];
        data.extend((0u8..=255).cycle().take(10_000));
        let (metadata, payload) = rans_encode(&data).unwrap();
        let decoded = rans_decode(&metadata, &payload, data.len()).unwrap();
        assert_eq!(decoded, data);
    }
}

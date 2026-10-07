use crate::{rans_decode, rans_encode};
use ace_core::{AceError, AceResult};

/// Number of independent scalar states used by the ACE 0.3 rANS4x container.
pub const RANS4X_LANES: usize = 4;
/// Metadata bytes used to store four normalized models and four payload lengths.
pub const RANS4X_METADATA_BYTES: usize = RANS4X_LANES * 512 + RANS4X_LANES * 4;

/// Encodes input into four deterministic round-robin rANS lanes.
///
/// This milestone keeps each lane wire-compatible with the proven scalar rANS coder while
/// removing the single long state dependency chain.  A later SIMD implementation can replace
/// lane execution without changing the surrounding entropy identifier or metadata contract.
pub fn rans4x_encode(input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    let mut lanes = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (index, &byte) in input.iter().enumerate() {
        lanes[index & 3].push(byte);
    }
    let mut metadata = Vec::with_capacity(RANS4X_METADATA_BYTES);
    let mut payload = Vec::new();
    let mut encoded_lanes = Vec::with_capacity(RANS4X_LANES);
    for lane in &lanes {
        let (model, encoded) = rans_encode(lane)?;
        encoded_lanes.push((model, encoded));
    }
    for (model, _) in &encoded_lanes {
        metadata.extend_from_slice(model);
    }
    for (_, encoded) in &encoded_lanes {
        let len = u32::try_from(encoded.len())
            .map_err(|_| AceError::InvalidRans("rANS4x lane payload exceeds u32"))?;
        metadata.extend_from_slice(&len.to_le_bytes());
    }
    for (_, encoded) in encoded_lanes {
        payload.extend_from_slice(&encoded);
    }
    Ok((metadata, payload))
}

/// Decodes four round-robin rANS lanes and restores the original byte order.
pub fn rans4x_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if metadata.len() != RANS4X_METADATA_BYTES {
        return Err(AceError::InvalidRans("invalid rANS4x metadata size"));
    }
    let model_bytes = RANS4X_LANES * 512;
    let mut payload_lengths = [0usize; RANS4X_LANES];
    for (lane, length) in payload_lengths.iter_mut().enumerate() {
        let start = model_bytes + lane * 4;
        *length = u32::from_le_bytes(
            metadata[start..start + 4]
                .try_into()
                .map_err(|_| AceError::InvalidRans("invalid rANS4x lane length"))?,
        ) as usize;
    }
    let total_payload = payload_lengths.iter().try_fold(0usize, |acc, &n| {
        acc.checked_add(n)
            .ok_or(AceError::InvalidRans("rANS4x payload length overflow"))
    })?;
    if total_payload != input.len() {
        return Err(AceError::InvalidRans(
            "rANS4x payload lengths do not match payload",
        ));
    }

    let mut decoded = Vec::with_capacity(RANS4X_LANES);
    let mut cursor = 0usize;
    for lane in 0..RANS4X_LANES {
        let model = &metadata[lane * 512..(lane + 1) * 512];
        let end = cursor + payload_lengths[lane];
        let lane_expected = (expected_size + (RANS4X_LANES - 1 - lane)) / RANS4X_LANES;
        decoded.push(rans_decode(model, &input[cursor..end], lane_expected)?);
        cursor = end;
    }

    let mut positions = [0usize; RANS4X_LANES];
    let mut output = Vec::with_capacity(expected_size);
    for index in 0..expected_size {
        let lane = index & 3;
        let pos = positions[lane];
        let byte = *decoded[lane]
            .get(pos)
            .ok_or(AceError::InvalidRans("rANS4x lane underflow"))?;
        positions[lane] += 1;
        output.push(byte);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies exact round-trip across uneven lane lengths.
    #[test]
    fn round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(100_003).collect();
        let (metadata, payload) = rans4x_encode(&data).unwrap();
        assert_eq!(
            rans4x_decode(&metadata, &payload, data.len()).unwrap(),
            data
        );
    }
}

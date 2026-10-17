// Decoder hardening (ACE 0.5.1): arithmetic on values read from a payload is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`); a proven-safe site carries a local
// `allow` with its proof.
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use crate::{rans_decode, rans_encode};
use ace_core::{AceError, AceResult};

/// Number of independent scalar states used by the ACE 0.3 rANS4x container.
pub const RANS4X_LANES: usize = 4;
/// Metadata bytes used to store four normalized models and four payload lengths.
pub const RANS4X_METADATA_BYTES: usize = RANS4X_LANES * 512 + RANS4X_LANES * 4;

/// Metadata bytes of one lane's normalized model.
const MODEL_BYTES_PER_LANE: usize = 512;
/// Metadata bytes of the four lane models (the four `u32` payload lengths follow).
const MODEL_BYTES: usize = RANS4X_LANES * MODEL_BYTES_PER_LANE;

/// Number of the `total` round-robin bytes that belong to `lane` (`ceil((total - lane) / 4)`,
/// computed without the `total + 3` overflow of the textbook formula).
fn lane_output_len(total: usize, lane: usize) -> usize {
    // `total / 4 + 1 <= usize::MAX`, so the addition cannot overflow.
    (total / RANS4X_LANES).saturating_add(usize::from(lane < total % RANS4X_LANES))
}

/// Encodes input into four deterministic round-robin rANS lanes.
///
/// This milestone keeps each lane wire-compatible with the proven scalar rANS coder while
/// removing the single long state dependency chain.  A later SIMD implementation can replace
/// lane execution without changing the surrounding entropy identifier or metadata contract.
///
/// # Errors
///
/// [`AceError::InvalidRans`] when a lane fails to encode or a lane payload exceeds `u32::MAX`
/// bytes.
#[allow(clippy::arithmetic_side_effects)] // encoder: only `index & 3` and slice lengths
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
///
/// # Errors
///
/// [`AceError::InvalidRans`] for metadata of the wrong length, lane lengths that do not add up
/// to the payload, or any lane error of [`crate::rans_decode`].
pub fn rans4x_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if metadata.len() != RANS4X_METADATA_BYTES {
        return Err(AceError::InvalidRans("invalid rANS4x metadata size"));
    }
    let (models, lengths) = metadata
        .split_at_checked(MODEL_BYTES)
        .ok_or(AceError::InvalidRans("invalid rANS4x metadata size"))?;
    let models = models.as_chunks::<MODEL_BYTES_PER_LANE>().0;
    let mut payload_lengths = [0usize; RANS4X_LANES];
    for (length, bytes) in payload_lengths.iter_mut().zip(lengths.as_chunks::<4>().0) {
        *length = usize::try_from(u32::from_le_bytes(*bytes))
            .map_err(|_| AceError::InvalidRans("invalid rANS4x lane length"))?;
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
    let mut remaining = input;
    for (lane, (model, &length)) in models.iter().zip(&payload_lengths).enumerate() {
        let (lane_input, rest) =
            remaining
                .split_at_checked(length)
                .ok_or(AceError::InvalidRans(
                    "rANS4x payload lengths do not match payload",
                ))?;
        remaining = rest;
        decoded.push(rans_decode(
            model,
            lane_input,
            lane_output_len(expected_size, lane),
        )?);
    }

    // Round-robin interleave. `rans_decode` returns exactly the length it was asked for, so the
    // lane lengths are fixed by `lane_output_len`; checking them once lets the copy below run
    // over whole 4-byte groups without a per-byte bounds or underflow test.
    let [lane0, lane1, lane2, lane3] = <[Vec<u8>; RANS4X_LANES]>::try_from(decoded)
        .map_err(|_| AceError::InvalidRans("rANS4x lane underflow"))?;
    let lanes = [&lane0, &lane1, &lane2, &lane3];
    for (lane, bytes) in lanes.iter().enumerate() {
        if bytes.len() != lane_output_len(expected_size, lane) {
            return Err(AceError::InvalidRans("rANS4x lane underflow"));
        }
    }
    let mut output = vec![0u8; expected_size];
    let (groups, tail) = output.as_chunks_mut::<RANS4X_LANES>();
    for ((group, (&a, &b)), (&c, &d)) in groups
        .iter_mut()
        .zip(lane0.iter().zip(&lane1))
        .zip(lane2.iter().zip(&lane3))
    {
        *group = [a, b, c, d];
    }
    // The 0..=3 bytes after the last full group come from the lanes' last elements.
    for (slot, lane) in tail.iter_mut().zip(lanes) {
        *slot = *lane
            .last()
            .ok_or(AceError::InvalidRans("rANS4x lane underflow"))?;
    }
    Ok(output)
}

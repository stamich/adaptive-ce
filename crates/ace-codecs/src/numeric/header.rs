//! The 40-byte NUM1 header: wire layout, validation and serialization.
//!
//! ```text
//! 0   4  magic  "NUM1"
//! 4   1  lane width in BYTES (2, 4 or 8; 2 added in 0.4.5)
//! 5   1  mode   (1 = FOR, 2 = Delta, 3 = DoD)
//! 6   1  bit width of every packed value (0 ..= lane bits)
//! 7   1  reserved (0)
//! 8   4  value count (u32 LE)
//! 12  4  tail byte count (u32 LE)
//! 16  8  first value / FOR base (u64 LE, must fit the lane)
//! 24  8  first delta (i64 LE; DoD mode only, otherwise 0)
//! 32  4  packed stream length in bytes (u32 LE)
//! 36  4  reserved (0)
//! 40  .. packed stream, then tail bytes
//! ```

use ace_core::{AceError, AceResult, NumericWidth};

use crate::NumericMode;

/// Four-byte magic that starts every NUM1 payload.
const MAGIC: [u8; 4] = *b"NUM1";

/// Fixed byte size of the self-describing NUM1 header.
pub const NUMERIC_HEADER_SIZE: usize = 40;

/// Header-level description of one NUM1 payload (everything except the packed values).
///
/// Produced by [`numeric_inspect`] without decoding any value; also the single input of the
/// serializer, so writer and reader share one definition of the header fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericPayloadInfo {
    /// Lane width of the stored integers.
    pub width: NumericWidth,
    /// Sub-codec used for the packed stream.
    pub mode: NumericMode,
    /// Bits per packed value (`0` means the stream is empty and fully implied by the header).
    pub bit_width: u8,
    /// Number of complete logical integers.
    pub value_count: usize,
    /// Number of verbatim trailing bytes.
    pub tail_bytes: usize,
    /// Length in bytes of the packed stream.
    pub packed_bytes: usize,
    /// First value (or FOR base).
    pub first_value: u64,
    /// First delta (DoD mode only; zero otherwise).
    pub first_delta: i64,
}

/// Inherent methods of [`NumericPayloadInfo`].
impl NumericPayloadInfo {
    /// Average stored bits per logical value, ignoring the header and tail.
    pub fn bits_per_value(&self) -> f64 {
        if self.value_count == 0 {
            0.0
        } else {
            self.packed_bytes as f64 * 8.0 / self.value_count as f64
        }
    }

    /// Number of bytes the payload decodes to (`value_count * width + tail`), overflow-checked.
    pub fn decoded_len(&self) -> AceResult<usize> {
        self.value_count
            .checked_mul(self.width.bytes())
            .and_then(|bytes| bytes.checked_add(self.tail_bytes))
            .ok_or(AceError::Malformed("numeric output size overflow"))
    }
}

/// Parses and fully validates the NUM1 header of `payload` without decoding any value.
///
/// Every check of [`crate::numeric_decode`] that does not need the expected output size is
/// performed here, so `ace inspect` reports exactly what the decoder would accept.
pub fn numeric_inspect(payload: &[u8]) -> AceResult<NumericPayloadInfo> {
    if payload.len() < NUMERIC_HEADER_SIZE {
        return Err(AceError::Malformed("truncated numeric header"));
    }
    if payload[0..4] != MAGIC {
        return Err(AceError::Malformed("invalid numeric payload magic"));
    }
    let width = NumericWidth::from_byte_width(payload[4])
        .ok_or(AceError::Malformed("unsupported numeric width"))?;
    let mode = NumericMode::try_from(payload[5])?;
    let bit_width = payload[6];
    if bit_width > width.bits() {
        return Err(AceError::Malformed("numeric bit width exceeds value width"));
    }
    let info = NumericPayloadInfo {
        width,
        mode,
        bit_width,
        value_count: read_u32_le(payload, 8),
        tail_bytes: read_u32_le(payload, 12),
        first_value: read_u64_le(payload, 16),
        first_delta: read_u64_le(payload, 24) as i64,
        packed_bytes: read_u32_le(payload, 32),
    };
    validate_lane_ranges(&info)?;

    let total = NUMERIC_HEADER_SIZE
        .checked_add(info.packed_bytes)
        .and_then(|len| len.checked_add(info.tail_bytes))
        .ok_or(AceError::Malformed("numeric length overflow"))?;
    if total != payload.len() {
        return Err(AceError::Malformed("numeric payload length mismatch"));
    }
    Ok(info)
}

/// Serializes `info` followed by the packed stream and the tail bytes.
///
/// `info.packed_bytes` / `info.tail_bytes` are ignored and taken from the slices, so a caller
/// cannot produce a header that disagrees with its body.
pub(crate) fn serialize_payload(
    info: &NumericPayloadInfo,
    packed: &[u8],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let as_u32 = |value: usize| {
        u32::try_from(value)
            .map_err(|_| AceError::ResourceLimitExceeded("numeric payload metadata"))
    };
    let mut out = Vec::with_capacity(NUMERIC_HEADER_SIZE + packed.len() + tail.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&[info.width.bytes() as u8, info.mode as u8, info.bit_width, 0]);
    out.extend_from_slice(&as_u32(info.value_count)?.to_le_bytes());
    out.extend_from_slice(&as_u32(tail.len())?.to_le_bytes());
    out.extend_from_slice(&info.first_value.to_le_bytes());
    out.extend_from_slice(&info.first_delta.to_le_bytes());
    out.extend_from_slice(&as_u32(packed.len())?.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    debug_assert_eq!(out.len(), NUMERIC_HEADER_SIZE);
    out.extend_from_slice(packed);
    out.extend_from_slice(tail);
    Ok(out)
}

/// Rejects first values / first deltas that a conforming encoder for this lane cannot emit.
fn validate_lane_ranges(info: &NumericPayloadInfo) -> AceResult<()> {
    let bits = info.width.bits();
    // The first value must fit the lane; otherwise decoding would silently truncate it.
    if bits < 64 && info.first_value >> bits != 0 {
        return Err(AceError::Malformed(
            "numeric first value exceeds lane width",
        ));
    }
    // Deltas are signed lane values; one extra bit of slack is allowed because the fixed-step
    // encoder stores the unsigned step of a monotonic sequence.
    if bits < 63 {
        let limit = 1i64 << bits;
        if info.first_delta <= -limit || info.first_delta >= limit {
            return Err(AceError::Malformed(
                "numeric first delta exceeds lane width",
            ));
        }
    }
    Ok(())
}

/// Reads a little-endian `u32` at `offset` (caller guarantees `offset + 4 <= len`).
fn read_u32_le(input: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(input[offset..offset + 4].try_into().expect("4-byte slice")) as usize
}

/// Reads a little-endian `u64` at `offset` (caller guarantees `offset + 8 <= len`).
fn read_u64_le(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(input[offset..offset + 8].try_into().expect("8-byte slice"))
}

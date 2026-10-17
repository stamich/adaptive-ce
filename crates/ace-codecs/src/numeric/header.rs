//! The 40-byte NUM1 header: wire layout, validation and serialization.
//!
//! ```text
//! 0   4  magic  "NUM1"
//! 4   1  lane width in BYTES (2, 4 or 8; 2 added in 0.4.5)
//! 5   1  mode   (1 = FOR, 2 = Delta, 3 = DoD, 4 = Patched since Format 1.5)
//! 6   1  bit width of every packed value (0 ..= lane bits)
//! 7   1  reserved (0); mode 4: the patched source mode (1, 2 or 3)
//! 8   4  value count (u32 LE)
//! 12  4  tail byte count (u32 LE)
//! 16  8  first value / FOR base (u64 LE, must fit the lane)
//! 24  8  first delta (i64 LE; DoD mode only, otherwise 0)
//! 32  4  packed stream length in bytes (u32 LE)
//! 36  4  reserved (0); mode 4: number of exceptions (u32 LE)
//! 40  .. packed stream, [mode 4: exception section,] then tail bytes
//! ```
//!
//! The exception section of a mode 4 payload (present only when the exception count is not
//! zero) is `h` (one byte, `1 ..= lane bits - bit width`), the exception positions bit-packed
//! at `bits(stream length - 1)` bits each, then the exception high parts bit-packed at `h`
//! bits each. A value of the patched stream is `low | high << bit width`, where `low` is the
//! packed slot and `high` is zero for every position that is not listed.

// Decoder hardening (ACE 0.5.1): arithmetic on values read from a payload is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_bitpack::{bits_required_u64, packed_len};
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
    /// Exception list of a [`NumericMode::Patched`] payload (`None` for every other mode).
    pub patch: Option<NumericPatch>,
}

/// The Patched FOR part of a NUM1 header (Format 1.5): which stream is patched and how many
/// values carry bits above the packed width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericPatch {
    /// Base mode whose stream is patched ([`NumericMode::FrameOfReference`],
    /// [`NumericMode::Delta`] or [`NumericMode::DeltaOfDelta`]).
    pub source: NumericMode,
    /// Number of stream values with a non-zero high part.
    pub exception_count: usize,
    /// Bits of each exception high part (`0` exactly when there are no exceptions).
    pub high_bits: u8,
    /// Length in bytes of the exception section.
    pub section_bytes: usize,
}

/// Inherent methods of [`NumericPatch`].
impl NumericPatch {
    /// Bits of one exception position for a source stream of `stream_len` values.
    pub fn position_bits(stream_len: usize) -> u8 {
        bits_required_u64(stream_len.saturating_sub(1) as u64)
    }

    /// Byte length of the exception section for `exception_count` exceptions of `high_bits`
    /// bits over a stream of `stream_len` values (`0` without exceptions).
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] when the length overflows `usize`.
    pub fn section_len(
        stream_len: usize,
        exception_count: usize,
        high_bits: u8,
    ) -> AceResult<usize> {
        if exception_count == 0 {
            return Ok(0);
        }
        packed_len(exception_count, Self::position_bits(stream_len))?
            .checked_add(packed_len(exception_count, high_bits)?)
            .and_then(|len| len.checked_add(1))
            .ok_or(AceError::Malformed("numeric exception section overflow"))
    }
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
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] when `value_count * width + tail` overflows `usize`.
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
///
/// # Errors
///
/// [`AceError::Malformed`] for a truncated header, a wrong magic or version, an unknown mode
/// or width, a bit width above the lane width, or section lengths that do not add up to the
/// payload length.
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
    let mut info = NumericPayloadInfo {
        width,
        mode,
        bit_width,
        patch: None,
        value_count: read_u32_le(payload, 8)?,
        tail_bytes: read_u32_le(payload, 12)?,
        first_value: read_u64_le(payload, 16),
        first_delta: i64::from_le_bytes(read_u64_le(payload, 24).to_le_bytes()),
        packed_bytes: read_u32_le(payload, 32)?,
    };
    validate_lane_ranges(&info)?;
    if mode == NumericMode::Patched {
        info.patch = Some(inspect_patch(payload, &info)?);
    }

    let section_bytes = info.patch.map_or(0, |patch| patch.section_bytes);
    let total = NUMERIC_HEADER_SIZE
        .checked_add(info.packed_bytes)
        .and_then(|len| len.checked_add(section_bytes))
        .and_then(|len| len.checked_add(info.tail_bytes))
        .ok_or(AceError::Malformed("numeric length overflow"))?;
    if total != payload.len() {
        return Err(AceError::Malformed("numeric payload length mismatch"));
    }
    Ok(info)
}

/// Validates the Patched FOR part of a header: source mode, exception count, the width byte of
/// the exception section and its length.
fn inspect_patch(payload: &[u8], info: &NumericPayloadInfo) -> AceResult<NumericPatch> {
    let source = NumericMode::try_from(payload[7])?;
    if source == NumericMode::Patched {
        return Err(AceError::Malformed("invalid numeric patch source"));
    }
    let exception_count = read_u32_le(payload, 36)?;
    let stream_len = source.stream_len(info.value_count);
    if exception_count > stream_len {
        return Err(AceError::Malformed(
            "numeric exception count exceeds stream length",
        ));
    }
    if exception_count == 0 {
        return Ok(NumericPatch {
            source,
            exception_count,
            high_bits: 0,
            section_bytes: 0,
        });
    }
    let high_bits = NUMERIC_HEADER_SIZE
        .checked_add(info.packed_bytes)
        .and_then(|start| payload.get(start))
        .copied()
        .ok_or(AceError::Malformed("truncated numeric exception section"))?;
    // `bit_width <= lane bits` was checked by the caller.
    let room = info.width.bits().saturating_sub(info.bit_width);
    if high_bits == 0 || high_bits > room {
        return Err(AceError::Malformed("invalid numeric exception width"));
    }
    Ok(NumericPatch {
        source,
        exception_count,
        high_bits,
        section_bytes: NumericPatch::section_len(stream_len, exception_count, high_bits)?,
    })
}

/// Serializes `info` followed by the packed stream and the tail bytes.
///
/// `info.packed_bytes` / `info.tail_bytes` are ignored and taken from the slices, so a caller
/// cannot produce a header that disagrees with its body. Patched payloads use
/// [`numeric_serialize_patched_payload`].
///
/// # Errors
///
/// [`AceError::ResourceLimitExceeded`] when a count or length does not fit the 32-bit header
/// fields; [`AceError::Malformed`] for an unsupported lane width or a Patched `info`.
pub fn numeric_serialize_payload(
    info: &NumericPayloadInfo,
    packed: &[u8],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    if info.mode == NumericMode::Patched || info.patch.is_some() {
        return Err(AceError::Malformed(
            "patched numeric payload needs numeric_serialize_patched_payload",
        ));
    }
    serialize(info, packed, &[], tail)
}

/// Serializes a [`NumericMode::Patched`] payload: header, packed low parts, exception section
/// and tail bytes.
///
/// `info.patch` supplies the source mode and the exception count; the section slice must be
/// exactly the length `info.patch` describes, so the header cannot disagree with the body.
///
/// # Errors
///
/// [`AceError::Malformed`] when `info` is not a Patched description or `section` does not match
/// it; otherwise the errors of [`numeric_serialize_payload`].
pub fn numeric_serialize_patched_payload(
    info: &NumericPayloadInfo,
    packed: &[u8],
    section: &[u8],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let Some(patch) = info.patch.filter(|_| info.mode == NumericMode::Patched) else {
        return Err(AceError::Malformed("not a patched numeric description"));
    };
    let first = section.first().copied().unwrap_or(0);
    if section.len() != patch.section_bytes || first != patch.high_bits {
        return Err(AceError::Malformed(
            "numeric exception section does not match its description",
        ));
    }
    serialize(info, packed, section, tail)
}

/// Writes the 40-byte header and the three body sections.
fn serialize(
    info: &NumericPayloadInfo,
    packed: &[u8],
    section: &[u8],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let as_u32 = |value: usize| {
        u32::try_from(value)
            .map_err(|_| AceError::ResourceLimitExceeded("numeric payload metadata"))
    };
    // Capacity hint only.
    let capacity = NUMERIC_HEADER_SIZE
        .saturating_add(packed.len())
        .saturating_add(section.len())
        .saturating_add(tail.len());
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(&MAGIC);
    let width = u8::try_from(info.width.bytes())
        .map_err(|_| AceError::Malformed("unsupported numeric width"))?;
    let source = info.patch.map_or(0, |patch| patch.source as u8);
    out.extend_from_slice(&[width, info.mode as u8, info.bit_width, source]);
    out.extend_from_slice(&as_u32(info.value_count)?.to_le_bytes());
    out.extend_from_slice(&as_u32(tail.len())?.to_le_bytes());
    out.extend_from_slice(&info.first_value.to_le_bytes());
    out.extend_from_slice(&info.first_delta.to_le_bytes());
    out.extend_from_slice(&as_u32(packed.len())?.to_le_bytes());
    let exceptions = info.patch.map_or(0, |patch| patch.exception_count);
    out.extend_from_slice(&as_u32(exceptions)?.to_le_bytes());
    debug_assert_eq!(out.len(), NUMERIC_HEADER_SIZE);
    out.extend_from_slice(packed);
    out.extend_from_slice(section);
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
    // `|first_delta| >= 2^bits` without negating (i64::MIN has no positive counterpart).
    if bits < 63 && info.first_delta.unsigned_abs() >= 1u64 << bits {
        return Err(AceError::Malformed(
            "numeric first delta exceeds lane width",
        ));
    }
    Ok(())
}

/// Reads a little-endian `u32` field at `offset` as a `usize` (caller guarantees
/// `offset + 4 <= len`; a value that does not fit `usize` is rejected, never truncated).
fn read_u32_le(input: &[u8], offset: usize) -> AceResult<usize> {
    usize::try_from(ace_core::read_lane::<4>(&input[offset..]))
        .map_err(|_| AceError::ResourceLimitExceeded("numeric header field"))
}

/// Reads a little-endian `u64` at `offset` (caller guarantees `offset + 8 <= len`).
fn read_u64_le(input: &[u8], offset: usize) -> u64 {
    ace_core::read_lane::<8>(&input[offset..])
}

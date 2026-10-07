//! Private little-endian bit I/O shared by the packers in `scalar`.

use ace_core::{AceError, AceResult};

/// Writes the low `width` bits of `value` at bit offset `bit_pos` (little-endian bit order).
///
/// Works one byte-aligned fragment at a time (at most nine fragments for 64 bits). The caller
/// guarantees that `value` fits in `width` bits and that `out` is large enough.
#[inline]
pub(crate) fn write_bits(out: &mut [u8], bit_pos: usize, value: u64, width: u8) {
    let mut remaining = width as usize;
    let mut position = bit_pos;
    let mut pending = value;
    while remaining > 0 {
        let bit_offset = position % 8;
        // Number of bits that still fit into the current output byte.
        let take = (8 - bit_offset).min(remaining);
        let mask = ((1u16 << take) - 1) as u8;
        out[position / 8] |= ((pending as u8) & mask) << bit_offset;
        pending >>= take;
        position += take;
        remaining -= take;
    }
}

/// Reads `width` bits starting at bit offset `bit_pos`; mirrors [`write_bits`].
///
/// Returns an error (never panics) when the stream ends early.
#[inline]
pub(crate) fn read_bits(input: &[u8], bit_pos: usize, width: u8) -> AceResult<u64> {
    let mut value = 0u64;
    let mut collected = 0usize;
    let mut position = bit_pos;
    let wanted = width as usize;
    while collected < wanted {
        let byte = *input
            .get(position / 8)
            .ok_or(AceError::Malformed("truncated bitpack payload"))?;
        let bit_offset = position % 8;
        let take = (8 - bit_offset).min(wanted - collected);
        let mask = ((1u16 << take) - 1) as u8;
        value |= (((byte >> bit_offset) & mask) as u64) << collected;
        collected += take;
        position += take;
    }
    Ok(value)
}

/// Reads `width` bits at `bit_pos` from a stream whose length the caller has already
/// validated (so the read cannot run past the end).
///
/// Uses one unaligned 8-byte load when 8 bytes are available and `width + bit offset <= 64`,
/// otherwise falls back to [`read_bits`].
#[inline(always)]
pub(crate) fn read_bits_validated(input: &[u8], bit_pos: usize, width: u8) -> u64 {
    let byte = bit_pos / 8;
    let shift = (bit_pos % 8) as u32;
    if let Some(chunk) = input.get(byte..byte + 8) {
        if width as u32 + shift <= 64 {
            let word = u64::from_le_bytes(chunk.try_into().expect("8-byte slice"));
            let mask = u64::MAX >> (64 - width as u32);
            return (word >> shift) & mask;
        }
    }
    read_bits(input, bit_pos, width).expect("packed length validated by caller")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference one-bit-at-a-time writer used only to prove the fast writer is identical.
    fn reference_write_bits(out: &mut [u8], bit_pos: usize, value: u64, width: u8) {
        for bit in 0..width as usize {
            if (value >> bit) & 1 != 0 {
                let position = bit_pos + bit;
                out[position / 8] |= 1u8 << (position % 8);
            }
        }
    }

    /// Reference one-bit-at-a-time reader used only to prove the fast reader is identical.
    fn reference_read_bits(input: &[u8], bit_pos: usize, width: u8) -> u64 {
        (0..width as usize).fold(0u64, |value, bit| {
            let position = bit_pos + bit;
            value | ((((input[position / 8] >> (position % 8)) & 1) as u64) << bit)
        })
    }

    /// The fragment bit I/O must produce exactly the bytes of the bit-by-bit reference.
    #[test]
    fn fast_bit_io_matches_reference_for_all_widths_and_offsets() {
        let mut state = 0x1234_5678_9abc_def0u64;
        for width in 1u8..=64 {
            for offset in 0usize..16 {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let mask = if width == 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                };
                let value = state & mask;
                let (mut fast, mut slow) = (vec![0u8; 24], vec![0u8; 24]);
                write_bits(&mut fast, offset, value, width);
                reference_write_bits(&mut slow, offset, value, width);
                assert_eq!(fast, slow, "writer width={width} offset={offset}");
                assert_eq!(read_bits(&fast, offset, width).unwrap(), value);
                assert_eq!(read_bits_validated(&fast, offset, width), value);
                let tight = &fast[..(offset + width as usize).div_ceil(8)];
                assert_eq!(
                    read_bits_validated(tight, offset, width),
                    value,
                    "tail fallback"
                );
                assert_eq!(reference_read_bits(&fast, offset, width), value);
            }
        }
    }
}

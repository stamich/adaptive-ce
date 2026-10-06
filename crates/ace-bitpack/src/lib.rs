//! Scalar integer transforms and bit packing used by ACE 0.4 numeric compression.
//!
//! The crate is intentionally format-agnostic: it operates on integer slices and returns
//! deterministic byte vectors. Format 1.3 framing is implemented by `ace-codecs`/`ace-format`.

use ace_core::{AceError, AceResult};

/// ZigZag-encodes a signed 32-bit value so small magnitudes map to small unsigned integers.
pub fn zigzag_i32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

/// Reverses [`zigzag_i32`].
pub fn unzigzag_u32(value: u32) -> i32 {
    ((value >> 1) as i32) ^ (-((value & 1) as i32))
}

/// ZigZag-encodes a signed 64-bit value so small magnitudes map to small unsigned integers.
pub fn zigzag_i64(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// Reverses [`zigzag_i64`].
pub fn unzigzag_u64(value: u64) -> i64 {
    ((value >> 1) as i64) ^ (-((value & 1) as i64))
}

/// Returns the minimum number of bits required to represent `value`.
pub fn bits_required_u64(value: u64) -> u8 {
    if value == 0 {
        0
    } else {
        (64 - value.leading_zeros()) as u8
    }
}

/// Returns the maximum bit width required by a 32-bit value slice.
pub fn max_bit_width_u32(values: &[u32]) -> u8 {
    values
        .iter()
        .copied()
        .map(|v| bits_required_u64(v as u64))
        .max()
        .unwrap_or(0)
}

/// Returns the maximum bit width required by a 64-bit value slice.
pub fn max_bit_width_u64(values: &[u64]) -> u8 {
    values
        .iter()
        .copied()
        .map(bits_required_u64)
        .max()
        .unwrap_or(0)
}

/// Packs unsigned 32-bit integers at a fixed bit width in little-endian bit order.
pub fn pack_u32(values: &[u32], bit_width: u8) -> AceResult<Vec<u8>> {
    if bit_width > 32 {
        return Err(AceError::Malformed("u32 bit width exceeds 32"));
    }
    let total_bits = values
        .len()
        .checked_mul(bit_width as usize)
        .ok_or(AceError::Malformed("bitpack size overflow"))?;
    let mut out = vec![0u8; (total_bits + 7) / 8];
    if bit_width == 0 {
        if values.iter().any(|&value| value != 0) {
            return Err(AceError::Malformed(
                "non-zero u32 value cannot use zero bit width",
            ));
        }
        return Ok(out);
    }
    let mask = if bit_width == 32 {
        u64::from(u32::MAX)
    } else {
        (1u64 << bit_width) - 1
    };
    let mut bit_pos = 0usize;
    for &value in values {
        if (value as u64) & !mask != 0 {
            return Err(AceError::Malformed("u32 value exceeds selected bit width"));
        }
        write_bits(&mut out, bit_pos, value as u64, bit_width);
        bit_pos += bit_width as usize;
    }
    Ok(out)
}

/// Unpacks `count` unsigned 32-bit integers from fixed-width little-endian bit order.
pub fn unpack_u32(input: &[u8], count: usize, bit_width: u8) -> AceResult<Vec<u32>> {
    if bit_width > 32 {
        return Err(AceError::Malformed("u32 bit width exceeds 32"));
    }
    validate_packed_len(input.len(), count, bit_width)?;
    let mut out = Vec::with_capacity(count);
    let mut bit_pos = 0usize;
    for _ in 0..count {
        out.push(read_bits(input, bit_pos, bit_width)? as u32);
        bit_pos += bit_width as usize;
    }
    Ok(out)
}

/// Packs unsigned 64-bit integers at a fixed bit width in little-endian bit order.
pub fn pack_u64(values: &[u64], bit_width: u8) -> AceResult<Vec<u8>> {
    if bit_width > 64 {
        return Err(AceError::Malformed("u64 bit width exceeds 64"));
    }
    let total_bits = values
        .len()
        .checked_mul(bit_width as usize)
        .ok_or(AceError::Malformed("bitpack size overflow"))?;
    let mut out = vec![0u8; (total_bits + 7) / 8];
    if bit_width == 0 {
        if values.iter().any(|&value| value != 0) {
            return Err(AceError::Malformed(
                "non-zero u64 value cannot use zero bit width",
            ));
        }
        return Ok(out);
    }
    let mask = if bit_width == 64 {
        u64::MAX
    } else {
        (1u64 << bit_width) - 1
    };
    let mut bit_pos = 0usize;
    for &value in values {
        if value & !mask != 0 {
            return Err(AceError::Malformed("u64 value exceeds selected bit width"));
        }
        write_bits(&mut out, bit_pos, value, bit_width);
        bit_pos += bit_width as usize;
    }
    Ok(out)
}

/// Unpacks `count` unsigned 64-bit integers from fixed-width little-endian bit order.
pub fn unpack_u64(input: &[u8], count: usize, bit_width: u8) -> AceResult<Vec<u64>> {
    if bit_width > 64 {
        return Err(AceError::Malformed("u64 bit width exceeds 64"));
    }
    validate_packed_len(input.len(), count, bit_width)?;
    let mut out = Vec::with_capacity(count);
    let mut bit_pos = 0usize;
    for _ in 0..count {
        out.push(read_bits(input, bit_pos, bit_width)?);
        bit_pos += bit_width as usize;
    }
    Ok(out)
}

/// Computes wrapping first-order signed deltas for 32-bit values.
pub fn delta_i32(values: &[u32]) -> Vec<i32> {
    values
        .windows(2)
        .map(|pair| pair[1].wrapping_sub(pair[0]) as i32)
        .collect()
}

/// Reconstructs 32-bit values from a first value and wrapping signed deltas.
pub fn undelta_i32(first: u32, deltas: &[i32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(deltas.len() + 1);
    let mut current = first;
    out.push(current);
    for &delta in deltas {
        current = current.wrapping_add(delta as u32);
        out.push(current);
    }
    out
}

/// Computes signed delta-of-delta values for a 32-bit integer sequence.
pub fn delta_of_delta_i32(values: &[u32]) -> (i32, Vec<i32>) {
    if values.len() < 2 {
        return (0, Vec::new());
    }
    let first_delta = values[1].wrapping_sub(values[0]) as i32;
    let mut previous = first_delta;
    let mut out = Vec::with_capacity(values.len().saturating_sub(2));
    for pair in values[1..].windows(2) {
        let delta = pair[1].wrapping_sub(pair[0]) as i32;
        out.push(delta.wrapping_sub(previous));
        previous = delta;
    }
    (first_delta, out)
}

/// Reconstructs 32-bit values from first value, first delta and delta-of-delta stream.
pub fn undelta_of_delta_i32(first: u32, first_delta: i32, dod: &[i32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(dod.len() + 2);
    out.push(first);
    let mut delta = first_delta;
    let mut current = first.wrapping_add(delta as u32);
    out.push(current);
    for &change in dod {
        delta = delta.wrapping_add(change);
        current = current.wrapping_add(delta as u32);
        out.push(current);
    }
    out
}

/// Converts 32-bit values to frame-of-reference offsets from their minimum.
pub fn frame_of_reference_u32(values: &[u32]) -> (u32, Vec<u32>) {
    let base = values.iter().copied().min().unwrap_or(0);
    (base, values.iter().map(|&v| v.wrapping_sub(base)).collect())
}

/// Reconstructs 32-bit values from a frame-of-reference base and offsets.
pub fn unframe_of_reference_u32(base: u32, offsets: &[u32]) -> Vec<u32> {
    offsets
        .iter()
        .map(|&offset| base.wrapping_add(offset))
        .collect()
}

/// Validates that `input_len` is sufficient for one packed integer stream.
fn validate_packed_len(input_len: usize, count: usize, bit_width: u8) -> AceResult<()> {
    let bits = count
        .checked_mul(bit_width as usize)
        .ok_or(AceError::Malformed("bitpack size overflow"))?;
    let needed = (bits + 7) / 8;
    if input_len != needed {
        return Err(AceError::Malformed("bitpack payload length mismatch"));
    }
    Ok(())
}

/// Writes up to 64 bits into a byte vector using little-endian bit numbering.
fn write_bits(out: &mut [u8], bit_pos: usize, value: u64, width: u8) {
    for bit in 0..width as usize {
        if (value >> bit) & 1 != 0 {
            let position = bit_pos + bit;
            out[position / 8] |= 1u8 << (position % 8);
        }
    }
}

/// Reads up to 64 bits from a packed little-endian bit stream.
fn read_bits(input: &[u8], bit_pos: usize, width: u8) -> AceResult<u64> {
    let mut value = 0u64;
    for bit in 0..width as usize {
        let position = bit_pos + bit;
        let byte = input
            .get(position / 8)
            .ok_or(AceError::Malformed("truncated bitpack payload"))?;
        value |= (((byte >> (position % 8)) & 1) as u64) << bit;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies scalar u32 bit packing across edge widths.
    #[test]
    fn u32_pack_roundtrip() {
        for width in [0u8, 1, 4, 7, 16, 31, 32] {
            let max = if width == 0 {
                0
            } else if width == 32 {
                u32::MAX
            } else {
                (1u32 << width) - 1
            };
            let values = vec![0, max / 3, max / 2, max];
            let packed = pack_u32(&values, width).unwrap();
            assert_eq!(unpack_u32(&packed, values.len(), width).unwrap(), values);
        }
    }

    /// Verifies frame-of-reference roundtrip.
    #[test]
    fn for_roundtrip() {
        let values = vec![1_000_000, 1_000_003, 1_000_002, 1_000_007];
        let (base, offsets) = frame_of_reference_u32(&values);
        assert_eq!(unframe_of_reference_u32(base, &offsets), values);
    }

    /// Verifies delta-of-delta roundtrip for a monotonic counter.
    #[test]
    fn dod_roundtrip() {
        let values = vec![1000, 1003, 1006, 1009, 1012, 1016];
        let (first_delta, dod) = delta_of_delta_i32(&values);
        assert_eq!(undelta_of_delta_i32(values[0], first_delta, &dod), values);
    }
}

/// Computes wrapping first-order signed deltas for 64-bit values.
pub fn delta_i64(values: &[u64]) -> Vec<i64> {
    values
        .windows(2)
        .map(|pair| pair[1].wrapping_sub(pair[0]) as i64)
        .collect()
}

/// Reconstructs 64-bit values from a first value and wrapping signed deltas.
pub fn undelta_i64(first: u64, deltas: &[i64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(deltas.len() + 1);
    let mut current = first;
    out.push(current);
    for &delta in deltas {
        current = current.wrapping_add(delta as u64);
        out.push(current);
    }
    out
}

/// Computes signed delta-of-delta values for a 64-bit integer sequence.
pub fn delta_of_delta_i64(values: &[u64]) -> (i64, Vec<i64>) {
    if values.len() < 2 {
        return (0, Vec::new());
    }
    let first_delta = values[1].wrapping_sub(values[0]) as i64;
    let mut previous = first_delta;
    let mut out = Vec::with_capacity(values.len().saturating_sub(2));
    for pair in values[1..].windows(2) {
        let delta = pair[1].wrapping_sub(pair[0]) as i64;
        out.push(delta.wrapping_sub(previous));
        previous = delta;
    }
    (first_delta, out)
}

/// Reconstructs 64-bit values from first value, first delta and delta-of-delta stream.
pub fn undelta_of_delta_i64(first: u64, first_delta: i64, dod: &[i64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(dod.len() + 2);
    out.push(first);
    let mut delta = first_delta;
    let mut current = first.wrapping_add(delta as u64);
    out.push(current);
    for &change in dod {
        delta = delta.wrapping_add(change);
        current = current.wrapping_add(delta as u64);
        out.push(current);
    }
    out
}

/// Converts 64-bit values to frame-of-reference offsets from their minimum.
pub fn frame_of_reference_u64(values: &[u64]) -> (u64, Vec<u64>) {
    let base = values.iter().copied().min().unwrap_or(0);
    (base, values.iter().map(|&v| v.wrapping_sub(base)).collect())
}

/// Reconstructs 64-bit values from a frame-of-reference base and offsets.
pub fn unframe_of_reference_u64(base: u64, offsets: &[u64]) -> Vec<u64> {
    offsets
        .iter()
        .map(|&offset| base.wrapping_add(offset))
        .collect()
}

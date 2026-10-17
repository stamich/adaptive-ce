//! Chimp and Chimp128 (Liakos et al., VLDB 2022) for binary64 values: an evaluation-only
//! implementation behind the `chimp-evaluation` family (ACE 0.6.1).
//!
//! Neither codec is part of the ACE format. They exist so the decision "does Chimp belong in
//! TS1?" is made on measured codec-level sizes: [`chimp_encode`] / [`chimp128_encode`] write a
//! real bitstream and the decoders restore the exact bits (tested), so the sizes are those of
//! working codecs, not estimates.
//!
//! ```text
//! first value: 64 raw bits; then per value x = v XOR reference
//! Chimp     00  x == 0
//!           01  trailing zeros > 6:  lead:3  significant:6  bits
//!           10  same rounded leading zeros as before:  (64 - lead) bits
//!           11  new leading zeros:  lead:3  (64 - lead) bits
//! Chimp128  reference = the one of the last 128 values that shares the low 14 bits (found
//!           through a table), used when x has more than 13 trailing zeros:
//!           00  index:7            01  index:7  lead:3  significant:6  bits
//!           otherwise the previous value is the reference and the flags 10 / 11 apply.
//! ```
//!
//! `lead` is the leading-zero count rounded down to one of `0, 8, 12, 16, 18, 20, 22, 24`.

use ace_bitpack::{BitReader, BitWriter};

/// Rounded leading-zero values selectable by the 3-bit field.
const LEADING_VALUES: [u32; 8] = [0, 8, 12, 16, 18, 20, 22, 24];
/// Trailing zeros above which the significant-bits form (flag 01) is used by Chimp.
const CHIMP_TRAILING_THRESHOLD: u32 = 6;
/// Trailing zeros above which Chimp128 prefers a window reference over the previous value.
const CHIMP128_TRAILING_THRESHOLD: u32 = 13;
/// Values in the Chimp128 window.
const WINDOW: usize = 128;
/// Bits of a Chimp128 window index.
const INDEX_BITS: u32 = 7;
/// Mask of the low 14 bits that key the Chimp128 table.
const KEY_MASK: u64 = (1 << 14) - 1;
/// "No previous leading-zero count" marker.
const NO_LEADING: u32 = 65;

/// Rounded leading zeros and their 3-bit code for a leading-zero count.
fn round_leading(leading_zeros: u32) -> (u32, u64) {
    let code = LEADING_VALUES
        .iter()
        .rposition(|&value| value <= leading_zeros)
        .unwrap_or(0);
    (LEADING_VALUES[code], code as u64)
}

/// Why a Chimp stream was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChimpError;

/// Display for [`ChimpError`].
impl std::fmt::Display for ChimpError {
    /// Writes a fixed message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid Chimp stream")
    }
}

/// Error impl for [`ChimpError`].
impl std::error::Error for ChimpError {}

/// Writes the flag-10 / flag-11 form of a non-zero `x` whose reference is the previous value.
fn put_previous_form(out: &mut BitWriter, x: u64, stored_leading: &mut u32) {
    let (leading, code) = round_leading(x.leading_zeros());
    if leading == *stored_leading {
        out.write_bits(0b10, 2);
    } else {
        *stored_leading = leading;
        out.write_bits(0b11, 2);
        out.write_bits(code, 3);
    }
    out.write_bits(x, 64 - leading);
}

/// Writes the significant-bits form (flag 01) of a non-zero `x`; `index` is the Chimp128 window
/// index (`None` for Chimp).
fn put_significant_form(out: &mut BitWriter, x: u64, index: Option<u64>) {
    let (leading, code) = round_leading(x.leading_zeros());
    let trailing = x.trailing_zeros();
    let significant = 64 - leading - trailing;
    out.write_bits(0b01, 2);
    if let Some(index) = index {
        out.write_bits(index, INDEX_BITS);
    }
    out.write_bits(code, 3);
    out.write_bits(u64::from(significant), 6);
    out.write_bits(x >> trailing, significant);
}

/// Encodes `values` with Chimp; returns the stream and its length in bits.
pub fn chimp_encode(values: &[u64]) -> (Vec<u8>, u64) {
    let mut out = BitWriter::with_capacity(values.len() * 16 + 64);
    let Some((&first, rest)) = values.split_first() else {
        return out.finish();
    };
    out.write_bits(first, 64);
    let (mut previous, mut stored_leading) = (first, NO_LEADING);
    for &value in rest {
        let x = value ^ previous;
        if x == 0 {
            out.write_bits(0b00, 2);
            stored_leading = NO_LEADING;
        } else if x.trailing_zeros() > CHIMP_TRAILING_THRESHOLD {
            put_significant_form(&mut out, x, None);
            stored_leading = NO_LEADING;
        } else {
            put_previous_form(&mut out, x, &mut stored_leading);
        }
        previous = value;
    }
    out.finish()
}

/// Encodes `values` with Chimp128; returns the stream and its length in bits.
pub fn chimp128_encode(values: &[u64]) -> (Vec<u8>, u64) {
    let mut out = BitWriter::with_capacity(values.len() * 16 + 64);
    let Some((&first, rest)) = values.split_first() else {
        return out.finish();
    };
    out.write_bits(first, 64);
    let mut window = [0u64; WINDOW];
    let mut table = vec![usize::MAX; 1 << 14];
    window[0] = first;
    table[(first & KEY_MASK) as usize] = 0;
    let (mut previous, mut stored_leading) = (first, NO_LEADING);
    for (offset, &value) in rest.iter().enumerate() {
        let current = offset + 1;
        let key = (value & KEY_MASK) as usize;
        let candidate = table[key];
        let mut referenced = false;
        if candidate != usize::MAX && current - candidate < WINDOW {
            let slot = candidate % WINDOW;
            let x = value ^ window[slot];
            if x == 0 {
                out.write_bits(0b00, 2);
                out.write_bits(slot as u64, INDEX_BITS);
                stored_leading = NO_LEADING;
                referenced = true;
            } else if x.trailing_zeros() > CHIMP128_TRAILING_THRESHOLD {
                put_significant_form(&mut out, x, Some(slot as u64));
                stored_leading = NO_LEADING;
                referenced = true;
            }
        }
        if !referenced {
            let x = value ^ previous;
            if x == 0 {
                out.write_bits(0b00, 2);
                out.write_bits(((current - 1) % WINDOW) as u64, INDEX_BITS);
                stored_leading = NO_LEADING;
            } else {
                put_previous_form(&mut out, x, &mut stored_leading);
            }
        }
        window[current % WINDOW] = value;
        table[key] = current;
        previous = value;
    }
    out.finish()
}

/// Reads the 3-bit leading-zero code.
fn read_leading(reader: &mut BitReader<'_>) -> Result<u32, ChimpError> {
    let code = reader.read_bits(3).ok_or(ChimpError)? as usize;
    LEADING_VALUES.get(code).copied().ok_or(ChimpError)
}

/// Reads the significant-bits payload and rebuilds `x`.
fn read_significant(reader: &mut BitReader<'_>) -> Result<u64, ChimpError> {
    let leading = read_leading(reader)?;
    let significant = reader.read_bits(6).ok_or(ChimpError)? as u32;
    let trailing = 64u32
        .checked_sub(leading)
        .and_then(|bits| bits.checked_sub(significant))
        .filter(|_| significant > 0)
        .ok_or(ChimpError)?;
    let bits = reader.read_bits(significant).ok_or(ChimpError)?;
    Ok(bits << trailing)
}

/// Reads the flag-10 / flag-11 payload.
fn read_previous_form(
    reader: &mut BitReader<'_>,
    flag: u64,
    stored_leading: &mut u32,
) -> Result<u64, ChimpError> {
    if flag == 0b11 {
        *stored_leading = read_leading(reader)?;
    }
    if *stored_leading > 24 {
        return Err(ChimpError);
    }
    reader.read_bits(64 - *stored_leading).ok_or(ChimpError)
}

/// Decodes `count` values of a [`chimp_encode`] stream.
///
/// # Errors
///
/// [`ChimpError`] for a truncated stream or an impossible field.
pub fn chimp_decode(stream: &[u8], bits: u64, count: usize) -> Result<Vec<u64>, ChimpError> {
    let mut reader = BitReader::new(stream, bits).map_err(|_| ChimpError)?;
    let mut values = Vec::with_capacity(count.min(1 << 20));
    if count == 0 {
        return Ok(values);
    }
    let mut previous = reader.read_bits(64).ok_or(ChimpError)?;
    values.push(previous);
    let mut stored_leading = NO_LEADING;
    while values.len() < count {
        let flag = reader.read_bits(2).ok_or(ChimpError)?;
        let x = match flag {
            0b00 => {
                stored_leading = NO_LEADING;
                0
            }
            0b01 => {
                stored_leading = NO_LEADING;
                read_significant(&mut reader)?
            }
            _ => read_previous_form(&mut reader, flag, &mut stored_leading)?,
        };
        previous ^= x;
        values.push(previous);
    }
    Ok(values)
}

/// Decodes `count` values of a [`chimp128_encode`] stream.
///
/// # Errors
///
/// [`ChimpError`] for a truncated stream or an impossible field.
pub fn chimp128_decode(stream: &[u8], bits: u64, count: usize) -> Result<Vec<u64>, ChimpError> {
    let mut reader = BitReader::new(stream, bits).map_err(|_| ChimpError)?;
    let mut values = Vec::with_capacity(count.min(1 << 20));
    if count == 0 {
        return Ok(values);
    }
    let mut window = [0u64; WINDOW];
    let mut previous = reader.read_bits(64).ok_or(ChimpError)?;
    values.push(previous);
    window[0] = previous;
    let mut stored_leading = NO_LEADING;
    while values.len() < count {
        let current = values.len();
        let flag = reader.read_bits(2).ok_or(ChimpError)?;
        let value = match flag {
            0b00 | 0b01 => {
                let slot = reader.read_bits(INDEX_BITS).ok_or(ChimpError)? as usize;
                stored_leading = NO_LEADING;
                let x = if flag == 0b01 {
                    read_significant(&mut reader)?
                } else {
                    0
                };
                window[slot] ^ x
            }
            _ => previous ^ read_previous_form(&mut reader, flag, &mut stored_leading)?,
        };
        values.push(value);
        window[current % WINDOW] = value;
        previous = value;
    }
    Ok(values)
}

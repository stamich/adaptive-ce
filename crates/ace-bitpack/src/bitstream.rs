//! Little-endian (LSB-first) bit I/O: the fixed-width primitives used by `scalar` packing and
//! the sequential [`BitWriter`] / [`BitReader`] used by variable-length codecs (TS1).
//!
//! Bit order: bit `k` of the stream is bit `k % 8` of byte `k / 8`. Values are written low bit
//! first, so a value written with [`BitWriter::write_bits`] occupies exactly the bits that
//! [`write_bits`] would set at the same position — one wire convention for every ACE codec
//! except Huffman, whose canonical codes are MSB-first by definition.

use ace_core::{AceError, AceResult};

/// Writes the low `width` bits of `value` at bit offset `bit_pos` (little-endian bit order).
///
/// Works one byte-aligned fragment at a time (at most nine fragments for 64 bits). The caller
/// guarantees that `value` fits in `width` bits and that `out` is large enough.
#[inline]
pub fn write_bits(out: &mut [u8], bit_pos: usize, value: u64, width: u8) {
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

/// Reads `width` bits at `bit_pos` from a stream whose length the caller has already
/// validated (so the read cannot run past the end).
///
/// Uses one unaligned 8-byte load when 8 bytes are available and `width + bit offset <= 64`,
/// otherwise reads byte fragments. `width == 0` yields `0`.
#[inline(always)]
pub fn read_bits_validated(input: &[u8], bit_pos: usize, width: u8) -> u64 {
    if width == 0 {
        return 0;
    }
    let byte = bit_pos / 8;
    let shift = (bit_pos % 8) as u32;
    if let Some(chunk) = input.get(byte..byte + 8) {
        if width as u32 + shift <= 64 {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            let word = u64::from_le_bytes(word);
            let mask = u64::MAX >> (64 - width as u32);
            return (word >> shift) & mask;
        }
    }
    // Tail of the stream: byte-fragment loop. Bytes past the end read as zero, which cannot
    // happen for a length-validated stream and keeps this path panic-free.
    let mut value = 0u64;
    let mut collected = 0usize;
    let mut position = bit_pos;
    while collected < width as usize {
        let byte = input.get(position / 8).copied().unwrap_or(0);
        let bit_offset = position % 8;
        let take = (8 - bit_offset).min(width as usize - collected);
        let mask = ((1u16 << take) - 1) as u8;
        value |= (((byte >> bit_offset) & mask) as u64) << collected;
        collected += take;
        position += take;
    }
    value
}

/// Mask with the low `width` bits set (`width` in `0..=64`).
#[inline(always)]
fn low_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Sequential LSB-first bit writer with a 64-bit accumulator.
///
/// Whole 64-bit words are flushed at once; [`BitWriter::finish`] pads the last byte with zero
/// bits and returns the exact bit length, which variable-length formats store in their header.
#[derive(Debug, Default, Clone)]
pub struct BitWriter {
    /// Completed bytes.
    bytes: Vec<u8>,
    /// Pending bits, low bit first.
    accumulator: u64,
    /// Number of valid bits in `accumulator` (`0..64`).
    pending: u32,
}

/// Inherent methods of [`BitWriter`].
impl BitWriter {
    /// Creates a writer whose byte buffer can hold `capacity_bits` without reallocating.
    pub fn with_capacity(capacity_bits: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity_bits.div_ceil(8)),
            accumulator: 0,
            pending: 0,
        }
    }

    /// Appends the low `width` bits of `value` (`width` in `0..=64`; higher bits are ignored).
    #[inline]
    pub fn write_bits(&mut self, value: u64, width: u32) {
        let width = width.min(64);
        if width == 0 {
            return;
        }
        let value = value & low_mask(width);
        self.accumulator |= value << self.pending;
        let total = self.pending + width;
        if total < 64 {
            self.pending = total;
            return;
        }
        self.bytes
            .extend_from_slice(&self.accumulator.to_le_bytes());
        // Bits of `value` that did not fit into the flushed word.
        let consumed = 64 - self.pending;
        self.accumulator = if consumed >= 64 { 0 } else { value >> consumed };
        self.pending = total - 64;
    }

    /// Appends one bit.
    #[inline]
    pub fn write_bit(&mut self, bit: bool) {
        self.write_bits(bit as u64, 1);
    }

    /// Number of bits written so far.
    pub fn bit_len(&self) -> u64 {
        self.bytes.len() as u64 * 8 + self.pending as u64
    }

    /// Returns `(bytes, bit_len)`; the last byte is zero-padded.
    pub fn finish(mut self) -> (Vec<u8>, u64) {
        let bit_len = self.bit_len();
        let tail = self.pending.div_ceil(8) as usize;
        self.bytes
            .extend_from_slice(&self.accumulator.to_le_bytes()[..tail]);
        (self.bytes, bit_len)
    }
}

/// Sequential LSB-first bit reader over a stream of known bit length.
///
/// Every read is bounds-checked against the declared bit length and returns `None` past the
/// end, so decoders turn truncation into an error instead of reading padding or panicking.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    /// Stream bytes (at least `ceil(bit_len / 8)`).
    input: &'a [u8],
    /// Declared number of valid bits.
    bit_len: u64,
    /// Next bit position.
    position: u64,
}

/// Inherent methods of [`BitReader`].
impl<'a> BitReader<'a> {
    /// Creates a reader; fails when `bit_len` exceeds the bits available in `input` or when
    /// `input` has more than one byte of padding (`input.len() != ceil(bit_len / 8)`).
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] when `input.len() != ceil(bit_len / 8)`.
    pub fn new(input: &'a [u8], bit_len: u64) -> AceResult<Self> {
        if bit_len.div_ceil(8) != input.len() as u64 {
            return Err(AceError::Malformed("bitstream length mismatch"));
        }
        Ok(Self {
            input,
            bit_len,
            position: 0,
        })
    }

    /// Reads `width` bits (`0..=64`); `None` when fewer than `width` bits remain.
    #[inline]
    pub fn read_bits(&mut self, width: u32) -> Option<u64> {
        if width > 64 || self.remaining() < width as u64 {
            return None;
        }
        let value = read_bits_validated(self.input, self.position as usize, width as u8);
        self.position += width as u64;
        Some(value)
    }

    /// Reads one bit; `None` at the end of the stream.
    #[inline]
    pub fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|bit| bit != 0)
    }

    /// Bits not yet read.
    pub fn remaining(&self) -> u64 {
        self.bit_len - self.position
    }
}

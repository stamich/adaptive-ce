//! MSB-first bit writer and reader for the Huffman payload.

/// MSB-first bit writer with a 64-bit accumulator (byte-identical to a bit-at-a-time writer).
#[derive(Default)]
pub(crate) struct BitWriter {
    /// Completed bytes.
    bytes: Vec<u8>,
    /// Pending bits, right-aligned.
    accumulator: u64,
    /// Number of valid bits in `accumulator` (always < 8 between calls).
    pending: u32,
}

/// Inherent methods of [`BitWriter`].
impl BitWriter {
    /// Creates a writer whose output buffer can hold `capacity` bytes without reallocating.
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            ..Self::default()
        }
    }

    /// Appends the low `length` bits of `code`, most significant bit first (`length <= 32`).
    #[inline]
    pub(crate) fn write(&mut self, code: u32, length: u8) {
        self.accumulator = (self.accumulator << length) | code as u64;
        self.pending += length as u32;
        while self.pending >= 8 {
            self.pending -= 8;
            self.bytes.push((self.accumulator >> self.pending) as u8);
        }
        // Drop flushed bits so the accumulator never exceeds 7 + 32 valid bits.
        self.accumulator &= (1u64 << self.pending) - 1;
    }

    /// Zero-pads the last partial byte and returns the stream.
    pub(crate) fn finish(mut self) -> Vec<u8> {
        if self.pending != 0 {
            self.bytes
                .push((self.accumulator << (8 - self.pending)) as u8);
        }
        self.bytes
    }
}

/// MSB-first bit reader with a 64-bit look-ahead buffer.
///
/// `buffer` holds the next unread bits left-aligned (bit 63 = next bit); bits beyond the end of
/// the stream read as zero, so [`Self::peek`] never fails and callers compare code lengths
/// against [`Self::remaining`].
pub(crate) struct BitReader<'a> {
    /// Encoded stream.
    input: &'a [u8],
    /// Index of the next byte not yet loaded into `buffer`.
    next_byte: usize,
    /// Left-aligned look-ahead bits.
    buffer: u64,
    /// Number of valid bits in `buffer`.
    buffered: u32,
}

/// Inherent methods of [`BitReader`].
impl<'a> BitReader<'a> {
    /// Creates a reader positioned at the first bit.
    pub(crate) fn new(input: &'a [u8]) -> Self {
        let mut reader = Self {
            input,
            next_byte: 0,
            buffer: 0,
            buffered: 0,
        };
        reader.refill();
        reader
    }

    /// Tops the look-ahead buffer up to at least 57 valid bits (or the stream end).
    #[inline(always)]
    fn refill(&mut self) {
        if let Some(chunk) = self.input.get(self.next_byte..self.next_byte + 8) {
            // Fast path: load 8 bytes at once, keep as many whole bytes as fit.
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            let word = u64::from_be_bytes(word);
            let take = (64 - self.buffered) / 8;
            if take > 0 {
                let loaded = if take == 8 {
                    word
                } else {
                    word >> (64 - 8 * take)
                };
                self.buffer |= loaded << (64 - self.buffered - 8 * take);
                self.buffered += 8 * take;
                self.next_byte += take as usize;
            }
            return;
        }
        while self.buffered <= 56 && self.next_byte < self.input.len() {
            self.buffer |= (self.input[self.next_byte] as u64) << (56 - self.buffered);
            self.buffered += 8;
            self.next_byte += 1;
        }
    }

    /// Number of unread bits.
    #[inline(always)]
    pub(crate) fn remaining(&self) -> usize {
        self.buffered as usize + (self.input.len() - self.next_byte) * 8
    }

    /// Returns the next `count` bits (`1..=32`) without consuming them (zero-padded at the end).
    #[inline(always)]
    pub(crate) fn peek(&self, count: u8) -> u32 {
        (self.buffer >> (64 - count as u32)) as u32
    }

    /// Consumes `count <= 32` bits that the caller has already inspected via [`Self::peek`].
    #[inline(always)]
    pub(crate) fn skip(&mut self, count: u8) {
        let count = count as u32;
        debug_assert!(count <= self.buffered);
        self.buffer <<= count;
        self.buffered -= count;
        if self.buffered < 32 {
            self.refill();
        }
    }

    /// Reads one bit, or `None` at the end of the stream.
    #[inline]
    pub(crate) fn read_bit(&mut self) -> Option<u32> {
        if self.buffered == 0 {
            return None;
        }
        let bit = (self.buffer >> 63) as u32;
        self.skip(1);
        Some(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The accumulator writer matches a bit-at-a-time reference and the reader reads it back.
    #[test]
    fn writer_matches_reference_and_reader_roundtrips() {
        let codes: Vec<(u32, u8)> = (1..200u32)
            .map(|i| {
                (
                    i.wrapping_mul(2_654_435_761) >> (32 - (i % 32 + 1)),
                    (i % 32 + 1) as u8,
                )
            })
            .collect();
        let mut writer = BitWriter::default();
        let mut reference: Vec<u8> = Vec::new();
        let (mut current, mut used) = (0u8, 0u8);
        for &(code, length) in &codes {
            writer.write(code, length);
            for shift in (0..length).rev() {
                current = (current << 1) | ((code >> shift) & 1) as u8;
                used += 1;
                if used == 8 {
                    reference.push(current);
                    current = 0;
                    used = 0;
                }
            }
        }
        if used != 0 {
            reference.push(current << (8 - used));
        }
        let bytes = writer.finish();
        assert_eq!(bytes, reference);

        let mut reader = BitReader::new(&bytes);
        for &(code, length) in &codes {
            let mut value = 0u32;
            for _ in 0..length {
                value = (value << 1) | reader.read_bit().unwrap();
            }
            assert_eq!(value, code);
        }
        assert!(
            reader.read_bit().is_none() || reader.remaining() < 8,
            "only padding bits remain"
        );
        let mut reader = BitReader::new(&bytes);
        assert_eq!(
            reader.peek(11),
            u32::from_be_bytes([bytes[0], bytes[1], 0, 0]) >> 21
        );
        reader.skip(3);
        assert_eq!(reader.remaining(), bytes.len() * 8 - 3);
        assert_eq!(
            reader.peek(16),
            (u32::from_be_bytes([bytes[0], bytes[1], bytes[2], 0]) << 3) >> 16
        );
    }
}

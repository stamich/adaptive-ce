//! Bit sink abstraction shared by the TS1 encoders and their exact size counters.
//!
//! Each encoder is written once against [`BitSink`]; running it with a [`BitCounter`] yields
//! the exact stream length without allocating or writing, running it with
//! [`ace_bitpack::BitWriter`] produces the stream. This keeps estimator and encoder in lockstep
//! by construction (no second implementation of the bit layout).

use ace_bitpack::BitWriter;

/// Destination of LSB-first bit fields.
pub trait BitSink {
    /// Appends the low `width` bits of `value` (`width` in `0..=64`).
    fn put(&mut self, value: u64, width: u32);

    /// True when the sink wants no more bits (a size counter whose limit is passed). Encoders
    /// that check it per symbol may stop early; the default never asks them to.
    #[inline]
    fn exhausted(&self) -> bool {
        false
    }
}

/// Real writer.
impl BitSink for BitWriter {
    #[inline]
    fn put(&mut self, value: u64, width: u32) {
        self.write_bits(value, width);
    }
}

/// Counts bits without storing them.
#[derive(Debug, Default, Clone, Copy)]
pub struct BitCounter {
    /// Bits "written" so far.
    pub bits: u64,
}

/// Exact size counter.
impl BitSink for BitCounter {
    #[inline]
    fn put(&mut self, _value: u64, width: u32) {
        self.bits += width as u64;
    }
}

/// Counts bits like [`BitCounter`] and reports [`BitSink::exhausted`] once more than `limit`
/// bits were written (the count is then only a lower bound).
#[derive(Debug, Clone, Copy)]
pub struct LimitedBitCounter {
    /// Bits "written" so far.
    pub bits: u64,
    /// Bits after which the sink is exhausted.
    pub limit: u64,
}

/// Size counter with an early exit.
impl BitSink for LimitedBitCounter {
    #[inline]
    fn put(&mut self, _value: u64, width: u32) {
        self.bits += width as u64;
    }

    #[inline]
    fn exhausted(&self) -> bool {
        self.bits > self.limit
    }
}

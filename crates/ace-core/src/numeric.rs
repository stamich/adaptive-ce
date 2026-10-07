//! Numeric-structure model types shared by the analyzer, planner, codecs and CLI (ACE 0.4).

/// Integer lane widths recognized by the ACE 0.4 numeric analyzer and codec.
///
/// The width describes how a block of raw bytes is *interpreted* as a sequence of little-endian
/// unsigned integers. ACE never needs a schema: the analyzer scores each width and the planner
/// keeps the best interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum NumericWidth {
    /// Two-byte little-endian values (added in ACE 0.4.5).
    U16,
    /// Four-byte little-endian values.
    U32,
    /// Eight-byte little-endian values.
    U64,
}

/// Inherent methods of [`NumericWidth`].
impl NumericWidth {
    /// Returns the width in source bytes of one logical numeric value.
    pub fn bytes(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    /// Returns the width in bits of one logical numeric value.
    pub fn bits(self) -> u8 {
        (self.bytes() * 8) as u8
    }

    /// Parses the serialized NUM1 width byte, which stores the lane width in bytes.
    ///
    /// Returns `None` for every value other than 2, 4 or 8 so that decoders can reject
    /// malformed payloads before allocating anything.
    pub fn from_byte_width(bytes: u8) -> Option<Self> {
        match bytes {
            2 => Some(Self::U16),
            4 => Some(Self::U32),
            8 => Some(Self::U64),
            _ => None,
        }
    }

    /// Returns the largest ZigZag delta bit width still considered "small" for this lane.
    ///
    /// "Small" is defined relative to the lane: a delta that needs at most half of the lane's
    /// bits still lets bit-packing save at least 50% of the raw size. Before ACE 0.4.5 the
    /// prefilter used a fixed 16-bit limit, which wrongly rejected u64 timestamps whose deltas
    /// need 17–32 bits (for example nanosecond clocks with microsecond jitter).
    pub fn small_delta_bits(self) -> u8 {
        self.bits() / 2
    }

    /// Returns a mask selecting the low `bits()` bits of a zero-extended `u64`.
    pub fn mask(self) -> u64 {
        match self {
            Self::U64 => u64::MAX,
            other => (1u64 << other.bits()) - 1,
        }
    }

    /// Sign-extends the low `bits()` bits of `value` to a full `i64`.
    ///
    /// Upper bits of `value` are ignored, so the argument does not need to be pre-masked.
    pub fn sign_extend(self, value: u64) -> i64 {
        let shift = 64 - self.bits() as u32;
        ((value << shift) as i64) >> shift
    }

    /// Returns the signed difference `current - previous` computed in this lane's modular ring.
    ///
    /// Both arguments are zero-extended lane values. A wrapping 16-bit counter going from
    /// `65535` to `0` therefore has delta `+1`, not `-65535`.
    pub fn lane_delta(self, previous: u64, current: u64) -> i64 {
        self.sign_extend(current.wrapping_sub(previous))
    }
}

/// Reads one little-endian lane value of `B` bytes (2, 4 or 8), zero-extended to 64 bits.
///
/// `B` is a *const generic* so every hot loop that is instantiated per lane width compiles to a
/// single fixed-size load instead of a run-time-length `memcpy` per value. The 0.4.5 benchmark
/// showed that a run-time `width.bytes()` made numeric scans 2.5–3× slower.
#[inline(always)]
pub fn read_lane<const B: usize>(chunk: &[u8]) -> u64 {
    debug_assert!(B == 2 || B == 4 || B == 8);
    let mut bytes = [0u8; 8];
    bytes[..B].copy_from_slice(&chunk[..B]);
    u64::from_le_bytes(bytes)
}

/// Const-generic equivalent of [`NumericWidth::lane_delta`] for `B`-byte lanes.
///
/// Computes `current - previous` in the `8 * B`-bit modular ring and sign-extends the result.
#[inline(always)]
pub fn lane_delta_const<const B: usize>(previous: u64, current: u64) -> i64 {
    let shift = 64 - (B * 8) as u32;
    ((current.wrapping_sub(previous) << shift) as i64) >> shift
}

/// Byte order considered by numeric structure detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum NumericEndian {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first. Detection telemetry supports it; the 0.4 codec writes LE only.
    Big,
}

/// File-level block-size policy introduced by ACE 0.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSizePolicy {
    /// Uses `AceConfig::block_size` exactly, preserving ACE 0.3 behavior.
    Fixed,
    /// Selects one deterministic block size for the complete file before block processing begins.
    Auto,
}

/// Expected access pattern used by the ACE 0.4 block-size advisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessHint {
    /// Optimizes for long sequential scans and throughput.
    Sequential,
    /// Balances compression, throughput and random-access amplification.
    Balanced,
    /// Caps block size to protect point/range-read latency.
    RandomAccess,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Width helpers must agree with each other for every supported lane.
    #[test]
    fn width_helpers_are_consistent() {
        for (width, bytes) in [
            (NumericWidth::U16, 2usize),
            (NumericWidth::U32, 4),
            (NumericWidth::U64, 8),
        ] {
            assert_eq!(width.bytes(), bytes);
            assert_eq!(width.bits() as usize, bytes * 8);
            assert_eq!(NumericWidth::from_byte_width(bytes as u8), Some(width));
            assert_eq!(width.small_delta_bits() as usize, bytes * 4);
        }
    }

    /// Lane-ring arithmetic must wrap at the lane boundary instead of at 64 bits.
    #[test]
    fn lane_delta_wraps_in_the_lane_ring() {
        assert_eq!(NumericWidth::U16.lane_delta(65_535, 0), 1);
        assert_eq!(NumericWidth::U16.lane_delta(0, 65_535), -1);
        assert_eq!(NumericWidth::U32.lane_delta(u32::MAX as u64, 2), 3);
        assert_eq!(NumericWidth::U64.lane_delta(u64::MAX, 4), 5);
        assert_eq!(NumericWidth::U16.mask(), 0xFFFF);
        assert_eq!(NumericWidth::U32.mask(), 0xFFFF_FFFF);
        assert_eq!(NumericWidth::U64.mask(), u64::MAX);
        assert_eq!(NumericWidth::U16.sign_extend(0x8000), -32_768);
    }

    /// The const-generic helpers must agree with the run-time `NumericWidth` methods.
    #[test]
    fn const_helpers_match_runtime_helpers() {
        let bytes = [0x34u8, 0x12, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
        assert_eq!(read_lane::<2>(&bytes), 0x1234);
        assert_eq!(read_lane::<4>(&bytes), 0xEEFF_1234);
        assert_eq!(read_lane::<8>(&bytes), 0xAABB_CCDD_EEFF_1234);
        for (a, b) in [(0u64, 1u64), (65_535, 0), (0, 65_535), (123, 99_999)] {
            assert_eq!(lane_delta_const::<2>(a, b), NumericWidth::U16.lane_delta(a, b));
            assert_eq!(lane_delta_const::<4>(a, b), NumericWidth::U32.lane_delta(a, b));
            assert_eq!(lane_delta_const::<8>(a, b), NumericWidth::U64.lane_delta(a, b));
        }
    }

    /// Unsupported serialized widths must be rejected rather than guessed.
    #[test]
    fn unsupported_byte_widths_are_rejected() {
        for bad in [0u8, 1, 3, 5, 6, 7, 9, 16, 255] {
            assert_eq!(NumericWidth::from_byte_width(bad), None);
        }
    }
}

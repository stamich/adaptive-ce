/// Integer lane widths recognized by the ACE 0.4 numeric analyzer and codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum NumericWidth {
    /// Four-byte little-endian values.
    U32,
    /// Eight-byte little-endian values.
    U64,
}

impl NumericWidth {
    /// Returns the width in source bytes of one logical numeric value.
    pub fn bytes(self) -> usize {
        match self {
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }
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

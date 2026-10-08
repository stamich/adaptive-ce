//! TS1 mode identifiers and their lane widths.

use ace_core::{AceError, AceResult};

/// Encoding mode of a TS1 payload (one byte on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum TimeSeriesMode {
    /// Gorilla XOR over IEEE-754 binary64 values (8-byte lanes).
    GorillaF64 = 1,
    /// Gorilla XOR over IEEE-754 binary32 values (4-byte lanes).
    GorillaF32 = 2,
    /// Run lengths of unchanged values plus ZigZag deltas over u16 / u32 / u64 lanes.
    RunDelta = 3,
}

/// Inherent methods of [`TimeSeriesMode`].
impl TimeSeriesMode {
    /// Every mode in wire-id order.
    pub const ALL: [Self; 3] = [Self::GorillaF64, Self::GorillaF32, Self::RunDelta];

    /// Whether `lane_bytes` is a legal lane width for this mode.
    pub const fn accepts_lane_bytes(self, lane_bytes: u8) -> bool {
        match self {
            Self::GorillaF64 => lane_bytes == 8,
            Self::GorillaF32 => lane_bytes == 4,
            Self::RunDelta => matches!(lane_bytes, 2 | 4 | 8),
        }
    }

    /// Stable lower-case label used by `ace inspect` and benchmark telemetry.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GorillaF64 => "gorilla_f64",
            Self::GorillaF32 => "gorilla_f32",
            Self::RunDelta => "run_delta",
        }
    }
}

/// Parses the serialized one-byte TS1 mode, rejecting unknown and reserved values.
impl TryFrom<u8> for TimeSeriesMode {
    /// Error type returned for unknown identifiers.
    type Error = AceError;

    /// Converts a serialized mode id into a supported mode.
    fn try_from(value: u8) -> AceResult<Self> {
        match value {
            1 => Ok(Self::GorillaF64),
            2 => Ok(Self::GorillaF32),
            3 => Ok(Self::RunDelta),
            _ => Err(AceError::InvalidTimeSeries("unknown TS1 mode")),
        }
    }
}

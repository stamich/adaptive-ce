//! NUM1 sub-codec identifiers.

use ace_core::AceError;

/// Numeric sub-codec stored in byte 5 of the NUM1 header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum NumericMode {
    /// Stores offsets from the minimum value and bit-packs those offsets.
    FrameOfReference = 1,
    /// Stores ZigZag first-order deltas after the first value.
    Delta = 2,
    /// Stores first value, first delta, then ZigZag delta-of-delta values.
    DeltaOfDelta = 3,
}

/// Inherent methods of [`NumericMode`].
impl NumericMode {
    /// Every mode in serialized-identifier order (also the deterministic tie-break order).
    pub const ALL: [NumericMode; 3] = [Self::FrameOfReference, Self::Delta, Self::DeltaOfDelta];

    /// Returns a short stable label used by `ace inspect` and benchmark telemetry.
    pub fn label(self) -> &'static str {
        match self {
            Self::FrameOfReference => "FOR",
            Self::Delta => "Delta",
            Self::DeltaOfDelta => "DoD",
        }
    }

    /// Number of packed stream values this mode stores for `value_count` logical integers.
    ///
    /// FOR stores every value, Delta all but the first, DoD all but the first two.
    pub fn stream_len(self, value_count: usize) -> usize {
        match self {
            Self::FrameOfReference => value_count,
            Self::Delta => value_count.saturating_sub(1),
            Self::DeltaOfDelta => value_count.saturating_sub(2),
        }
    }
}

/// Parses the serialized one-byte identifier, rejecting unknown values.
impl TryFrom<u8> for NumericMode {
    /// Unknown identifiers are reported as malformed input.
    type Error = AceError;

    /// Parses one serialized numeric mode identifier.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::FrameOfReference),
            2 => Ok(Self::Delta),
            3 => Ok(Self::DeltaOfDelta),
            _ => Err(AceError::Malformed("unknown numeric mode")),
        }
    }
}

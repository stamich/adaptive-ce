/// Byte-oriented primary codec used by a physical compression plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum CodecId {
    /// Stores the transformed input bytes verbatim.
    Raw = 0,
    /// Packet run-length encoding.
    Rle = 1,
    /// LZ77-style literal/match token encoding.
    Lz = 2,
    /// Self-describing integer codec introduced by ACE Format 1.3.
    Numeric = 3,
    /// Self-describing TS1 time-series codec (Gorilla XOR floats, RunDelta integers)
    /// introduced by ACE Format 1.4.
    TimeSeries = 4,
}

/// Parses the serialized one-byte identifier of [`CodecId`], rejecting unknown values.
impl TryFrom<u8> for CodecId {
    /// Error type returned for unknown numeric identifiers.
    type Error = crate::AceError;

    /// Converts a serialized codec identifier into a supported codec.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Rle),
            2 => Ok(Self::Lz),
            3 => Ok(Self::Numeric),
            4 => Ok(Self::TimeSeries),
            other => Err(crate::AceError::UnsupportedCodec(other)),
        }
    }
}

/// Optional entropy coder applied after the primary codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum EntropyCodecId {
    /// No entropy coding is applied.
    None = 0,
    /// Canonical Huffman coding inherited from ACE 0.1.
    Huffman = 1,
    /// Scalar 32-bit range Asymmetric Numeral System introduced in ACE 0.2.
    Rans = 2,
    /// Four-lane rANS container introduced in ACE 0.3 to reduce serial dependency chains.
    Rans4x = 3,
}

/// Bytes of the little-endian `u32` primary-stream length stored in front of the entropy model
/// metadata of every block whose entropy coder is not [`EntropyCodecId::None`].
pub const PRIMARY_LENGTH_PREFIX_BYTES: usize = 4;

/// Inherent methods of [`EntropyCodecId`].
impl EntropyCodecId {
    /// Container bytes added in front of this coder's model metadata (`0` or
    /// [`PRIMARY_LENGTH_PREFIX_BYTES`]); the single source of that wire rule for encoder,
    /// decoder and planner cost estimates.
    pub const fn metadata_prefix_bytes(self) -> usize {
        match self {
            Self::None => 0,
            Self::Huffman | Self::Rans | Self::Rans4x => PRIMARY_LENGTH_PREFIX_BYTES,
        }
    }

    /// Stable lower-case label used in plan-distribution telemetry.
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Huffman => "huffman",
            Self::Rans => "rans",
            Self::Rans4x => "rans4x",
        }
    }
}

/// Parses the serialized one-byte identifier of [`EntropyCodecId`], rejecting unknown values.
impl TryFrom<u8> for EntropyCodecId {
    /// Error type returned for unknown numeric identifiers.
    type Error = crate::AceError;

    /// Converts a serialized entropy identifier into a supported entropy codec.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Huffman),
            2 => Ok(Self::Rans),
            3 => Ok(Self::Rans4x),
            other => Err(crate::AceError::UnsupportedEntropyCodec(other)),
        }
    }
}

/// Reversible byte transformation applied before the primary codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum TransformId {
    /// No transform. This identifier is reserved and is normally omitted from a plan.
    None = 0,
    /// Byte-wise delta transform using wrapping subtraction.
    DeltaByte = 1,
}

/// Parses the serialized one-byte identifier of [`TransformId`], rejecting unknown values.
impl TryFrom<u8> for TransformId {
    /// Error type returned for unknown numeric identifiers.
    type Error = crate::AceError;

    /// Converts a serialized transform identifier into a supported transform.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::DeltaByte),
            other => Err(crate::AceError::UnsupportedTransform(other)),
        }
    }
}

/// Encoder-side LZ search strategy. Both variants produce the same wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum LzMode {
    /// Checks only the most recent hashed candidate.
    Fast,
    /// Walks a bounded previous-position chain to find a longer match.
    Balanced,
}

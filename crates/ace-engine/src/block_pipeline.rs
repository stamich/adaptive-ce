//! Per-block pipeline framing shared by the encoder and both decoders.
//!
//! A block whose entropy coder is not `None` stores the primary-codec stream length as a
//! little-endian `u32` in front of the entropy model metadata
//! ([`ace_core::PRIMARY_LENGTH_PREFIX_BYTES`]). The wrap and split functions below are the only
//! code that knows this layout.

// Decoder hardening (ACE 0.5.1): arithmetic on values read from a file is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_codecs::{decode_codec, payload_minor_version};
use ace_core::{
    AceError, AceResult, CodecId, DecodeLimits, EntropyCodecId, FORMAT_MAJOR,
    PRIMARY_LENGTH_PREFIX_BYTES,
};
use ace_entropy::decode_entropy_cow;
use ace_format::{checksum, BlockHeader};
use ace_transforms::invert_transform;

/// Prepends the primary-stream length to entropy model metadata when the coder requires it.
///
/// # Errors
///
/// [`AceError::ResourceLimitExceeded`] when the primary stream is longer than `u32::MAX` bytes.
pub fn wrap_entropy_metadata(
    entropy: EntropyCodecId,
    primary_len: usize,
    metadata: Vec<u8>,
) -> AceResult<Vec<u8>> {
    if entropy.metadata_prefix_bytes() == 0 {
        return Ok(metadata);
    }
    let primary_len = u32::try_from(primary_len)
        .map_err(|_| AceError::ResourceLimitExceeded("primary codec stream size"))?;
    let mut wrapped =
        Vec::with_capacity(PRIMARY_LENGTH_PREFIX_BYTES.saturating_add(metadata.len()));
    wrapped.extend_from_slice(&primary_len.to_le_bytes());
    wrapped.extend_from_slice(&metadata);
    Ok(wrapped)
}

/// Minor format version a block needs, given its codec, entropy coder and stored payload.
///
/// A payload that is stored directly (entropy `None`) is inspected: a NUM1 or TS1 stream of a
/// Format 1.5 mode needs 1.5, a TS1 stream 1.4, anything else 1.3. The primary stream of an
/// entropy-coded block cannot be seen without decoding it, so a NUM1 block with an entropy
/// stage conservatively declares 1.5; declaring a higher minor version than needed is always
/// valid, and the planner does not combine NUM1 with entropy coders.
#[must_use]
pub fn block_format_minor(codec: CodecId, entropy: EntropyCodecId, payload: &[u8]) -> u8 {
    let needed = match entropy {
        EntropyCodecId::None => payload_minor_version(codec, payload),
        _ if matches!(codec, CodecId::Numeric | CodecId::TimeSeries) => 5,
        _ => 0,
    };
    needed.max(ace_format::FORMAT_MINOR_BASE)
}

/// Splits stored block metadata into `(entropy model metadata, primary stream length)`.
///
/// # Errors
///
/// [`AceError::Malformed`] when the coder requires a length prefix and the metadata is shorter
/// than the prefix; [`AceError::ResourceLimitExceeded`] when the length does not fit `usize`.
pub fn split_entropy_metadata(
    entropy: EntropyCodecId,
    metadata: &[u8],
    payload_len: usize,
) -> AceResult<(&[u8], usize)> {
    if entropy.metadata_prefix_bytes() == 0 {
        return Ok((metadata, payload_len));
    }
    if metadata.len() < PRIMARY_LENGTH_PREFIX_BYTES {
        return Err(AceError::Malformed("missing primary-stream length"));
    }
    let (prefix, model) = metadata.split_at(PRIMARY_LENGTH_PREFIX_BYTES);
    let primary_len = usize::try_from(ace_core::read_lane::<PRIMARY_LENGTH_PREFIX_BYTES>(prefix))
        .map_err(|_| AceError::ResourceLimitExceeded("primary stream size"))?;
    Ok((model, primary_len))
}

/// Largest primary (codec) stream a block of `original_size` bytes can have.
///
/// Every codec's worst case is a small expansion of its input (LZ and RLE add one header byte
/// per 128 literals, NUM1 and TS1 a fixed header), so twice the block plus 1 KiB is never
/// reached by an encoder. The bound matters for entropy-coded blocks: their primary length is
/// a 32-bit field, and a rANS model with a probability-one symbol decodes any number of
/// symbols from four bytes — without the bound a 50-byte block could make the decoder produce
/// 4 GiB before the codec stage rejects it.
fn max_primary_len(original_size: usize) -> usize {
    original_size.saturating_mul(2).saturating_add(1024)
}

/// Decodes one parsed block (entropy → codec → inverse transforms) and verifies its CRC32C.
///
/// `minor_version` is the minor format version of the file: a payload that uses a feature
/// newer than the file declares (for example a NUM1 mode 4 stream in a 1.4 file) is rejected
/// with [`AceError::UnsupportedVersion`] before it is decoded.
pub(crate) fn decode_encoded_block(
    header: &BlockHeader,
    metadata: &[u8],
    payload: &[u8],
    minor_version: u8,
    limits: &DecodeLimits,
) -> AceResult<Vec<u8>> {
    let original_size = header.original_size as usize;
    if original_size > limits.max_block_size {
        return Err(AceError::ResourceLimitExceeded("block output size"));
    }
    if let Some(dictionary) = header.dictionary {
        return Err(AceError::MissingDictionary(dictionary.id.0));
    }
    let (entropy_metadata, primary_size) =
        split_entropy_metadata(header.entropy, metadata, payload.len())?;
    if primary_size > max_primary_len(original_size) {
        return Err(AceError::ResourceLimitExceeded("primary stream size"));
    }
    // `None` entropy borrows the payload, so a RAW block is copied once, not twice.
    let primary = decode_entropy_cow(header.entropy, entropy_metadata, payload, primary_size)?;
    if payload_minor_version(header.codec, &primary) > minor_version {
        return Err(AceError::UnsupportedVersion {
            major: FORMAT_MAJOR,
            minor: minor_version,
        });
    }
    let mut stage = decode_codec(header.codec, &primary, original_size)?;
    for &transform in header.transforms.iter().rev() {
        stage = invert_transform(transform, &stage)?;
    }
    if stage.len() != original_size {
        return Err(AceError::Malformed("decoded block size mismatch"));
    }
    if checksum(&stage) != header.payload_crc32c {
        return Err(AceError::ChecksumMismatch(header.block_id));
    }
    Ok(stage)
}

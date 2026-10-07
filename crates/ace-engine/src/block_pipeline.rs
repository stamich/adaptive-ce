//! Per-block pipeline framing shared by the encoder and both decoders.
//!
//! A block whose entropy coder is not `None` stores the primary-codec stream length as a
//! little-endian `u32` in front of the entropy model metadata
//! ([`ace_core::PRIMARY_LENGTH_PREFIX_BYTES`]). The wrap and split functions below are the only
//! code that knows this layout.

use ace_codecs::decode_codec;
use ace_core::{AceError, AceResult, DecodeLimits, EntropyCodecId, PRIMARY_LENGTH_PREFIX_BYTES};
use ace_entropy::decode_entropy_cow;
use ace_format::{checksum, BlockHeader};
use ace_transforms::invert_transform;

/// Prepends the primary-stream length to entropy model metadata when the coder requires it.
pub(crate) fn wrap_entropy_metadata(
    entropy: EntropyCodecId,
    primary_len: usize,
    metadata: Vec<u8>,
) -> AceResult<Vec<u8>> {
    if entropy.metadata_prefix_bytes() == 0 {
        return Ok(metadata);
    }
    let primary_len = u32::try_from(primary_len)
        .map_err(|_| AceError::ResourceLimitExceeded("primary codec stream size"))?;
    let mut wrapped = Vec::with_capacity(PRIMARY_LENGTH_PREFIX_BYTES + metadata.len());
    wrapped.extend_from_slice(&primary_len.to_le_bytes());
    wrapped.extend_from_slice(&metadata);
    Ok(wrapped)
}

/// Splits stored block metadata into `(entropy model metadata, primary stream length)`.
fn split_entropy_metadata(
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
    let primary_len = u32::from_le_bytes(prefix.try_into().expect("prefix has exactly 4 bytes"));
    Ok((model, primary_len as usize))
}

/// Decodes one parsed block (entropy → codec → inverse transforms) and verifies its CRC32C.
pub(crate) fn decode_encoded_block(
    header: &BlockHeader,
    metadata: &[u8],
    payload: &[u8],
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
    // `None` entropy borrows the payload, so a RAW block is copied once, not twice.
    let primary = decode_entropy_cow(header.entropy, entropy_metadata, payload, primary_size)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Wrap and split are inverse; `None` carries no prefix; short metadata is rejected.
    #[test]
    fn entropy_metadata_framing() {
        let wrapped = wrap_entropy_metadata(EntropyCodecId::Huffman, 1234, vec![7, 8]).unwrap();
        assert_eq!(wrapped.len(), PRIMARY_LENGTH_PREFIX_BYTES + 2);
        assert_eq!(
            split_entropy_metadata(EntropyCodecId::Huffman, &wrapped, 99).unwrap(),
            (&[7u8, 8][..], 1234)
        );
        assert_eq!(
            wrap_entropy_metadata(EntropyCodecId::None, 5, vec![1]).unwrap(),
            vec![1]
        );
        assert_eq!(
            split_entropy_metadata(EntropyCodecId::None, &[], 42)
                .unwrap()
                .1,
            42
        );
        assert!(split_entropy_metadata(EntropyCodecId::Rans, &[1, 2], 0).is_err());
    }
}

use ace_core::{CodecId, EntropyCodecId};
use ace_format::{decode_block_header, encode_block_header, BlockHeader, BLOCK_HEADER_SIZE};

/// Verifies Format 1.3 serializes and parses the new numeric primary codec.
#[test]
fn numeric_codec_roundtrips_in_1_3_block_header() {
    let header = BlockHeader {
        block_id: 7,
        original_size: 262_144,
        encoded_size: 4096,
        metadata_size: 0,
        codec: CodecId::Numeric,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: 123,
    };
    let bytes = encode_block_header(&header);
    let decoded =
        decode_block_header(&bytes[..BLOCK_HEADER_SIZE], &bytes[BLOCK_HEADER_SIZE..], 3).unwrap();
    assert_eq!(decoded.codec, CodecId::Numeric);
}

/// Verifies older format versions reject the codec identifier they cannot interpret.
#[test]
fn numeric_codec_requires_format_1_3() {
    let header = BlockHeader {
        block_id: 0,
        original_size: 16,
        encoded_size: 8,
        metadata_size: 0,
        codec: CodecId::Numeric,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: 0,
    };
    let bytes = encode_block_header(&header);
    assert!(
        decode_block_header(&bytes[..BLOCK_HEADER_SIZE], &bytes[BLOCK_HEADER_SIZE..], 2).is_err()
    );
}

//! Tests of `ace_engine` `block_pipeline` (formerly the inline unit tests of `src`).

use ace_core::{EntropyCodecId, PRIMARY_LENGTH_PREFIX_BYTES};
use ace_engine::*;

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

/// A block declares the version its stored payload needs; entropy-coded NUM1 is conservative.
#[test]
fn block_format_minor_follows_the_stored_payload() {
    use ace_core::CodecId;
    let mut num1 = b"NUM1".to_vec();
    num1.extend_from_slice(&[4, 3, 0, 0]);
    let mut ts1 = b"TS1\0".to_vec();
    ts1.extend_from_slice(&[1, 3, 8, 0]);
    let none = EntropyCodecId::None;
    assert_eq!(block_format_minor(CodecId::Raw, none, &[1, 2, 3]), 3);
    assert_eq!(
        block_format_minor(CodecId::Lz, EntropyCodecId::Rans, &[]),
        3
    );
    assert_eq!(block_format_minor(CodecId::Numeric, none, &num1), 3);
    assert_eq!(block_format_minor(CodecId::TimeSeries, none, &ts1), 4);
    num1[5] = 4;
    ts1[5] = 4;
    assert_eq!(block_format_minor(CodecId::Numeric, none, &num1), 5);
    assert_eq!(block_format_minor(CodecId::TimeSeries, none, &ts1), 5);
    assert_eq!(
        block_format_minor(CodecId::Numeric, EntropyCodecId::Huffman, &[]),
        5
    );
}

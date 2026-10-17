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

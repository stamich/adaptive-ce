//! Tests of `ace_entropy` `dispatch` (formerly the inline unit tests of `src`).

use ace_core::EntropyCodecId;
use ace_entropy::*;
use std::borrow::Cow;

/// Every entropy coder round-trips through the dispatcher; `None` borrows.
#[test]
fn dispatch_roundtrip() {
    let data: Vec<u8> = (0..20_000u32).map(|i| (i % 7 + i % 3) as u8).collect();
    for id in [
        EntropyCodecId::None,
        EntropyCodecId::Huffman,
        EntropyCodecId::Rans,
        EntropyCodecId::Rans4x,
    ] {
        let (metadata, payload) = encode_entropy(id, &data).unwrap();
        assert_eq!(
            decode_entropy(id, &metadata, &payload, data.len()).unwrap(),
            data
        );
    }
    assert!(matches!(
        decode_entropy_cow(EntropyCodecId::None, &[], &data, data.len()).unwrap(),
        Cow::Borrowed(_)
    ));
    assert!(decode_entropy(EntropyCodecId::None, &[], &data, data.len() + 1).is_err());
}

//! Tests of `ace_codecs` `dispatch` (formerly the inline unit tests of `src`).

use ace_codecs::*;
use ace_core::{CodecId, LzMode};

/// Every codec round-trips through the dispatcher.
#[test]
fn dispatch_roundtrip_for_every_codec() {
    let data: Vec<u8> = (0..4096u32)
        .flat_map(|i| (1000 + 3 * i).to_le_bytes())
        .collect();
    for id in [
        CodecId::Raw,
        CodecId::Rle,
        CodecId::Lz,
        CodecId::Numeric,
        CodecId::TimeSeries,
    ] {
        for mode in [None, Some(LzMode::Balanced)] {
            let encoded = encode_codec(id, mode, &data).unwrap();
            assert_eq!(
                decode_codec(id, &encoded, data.len()).unwrap(),
                data,
                "{id:?}"
            );
        }
    }
}

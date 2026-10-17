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

/// The minor format version of a payload follows its codec and, for NUM1 and TS1, the mode byte.
#[test]
fn payload_minor_version_follows_codec_and_mode() {
    let data: Vec<u8> = (0..4096u32)
        .flat_map(|i| (1000 + 3 * i).to_le_bytes())
        .collect();
    for id in [CodecId::Raw, CodecId::Rle, CodecId::Lz] {
        let encoded = encode_codec(id, None, &data).unwrap();
        assert_eq!(payload_minor_version(id, &encoded), 0, "{id:?}");
    }
    let num1 = encode_codec(CodecId::Numeric, None, &data).unwrap();
    assert_eq!(payload_minor_version(CodecId::Numeric, &num1), 3);
    let ts1 = encode_codec(CodecId::TimeSeries, None, &data).unwrap();
    assert_eq!(payload_minor_version(CodecId::TimeSeries, &ts1), 4);

    // Mode 4 is the first mode of Format 1.5; the decoders of this crate may not know it yet,
    // but the version a stream needs is a property of its header alone.
    let mut forged = num1.clone();
    forged[5] = 4;
    assert_eq!(payload_minor_version(CodecId::Numeric, &forged), 5);
    let mut forged = ts1.clone();
    forged[5] = 4;
    assert_eq!(payload_minor_version(CodecId::TimeSeries, &forged), 5);

    // No magic, no mode byte: the codec's base version.
    assert_eq!(payload_minor_version(CodecId::Numeric, b"NUM"), 3);
    assert_eq!(payload_minor_version(CodecId::TimeSeries, &[]), 4);
    let mut other_magic = num1;
    other_magic[0] = b'X';
    other_magic[5] = 4;
    assert_eq!(payload_minor_version(CodecId::Numeric, &other_magic), 3);
}

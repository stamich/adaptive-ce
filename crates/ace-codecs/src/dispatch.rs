//! `CodecId` → codec dispatch (the only place that knows every primary codec).

use ace_core::{AceResult, CodecId, LzMode};

use crate::{
    lz_decode, lz_encode, numeric_decode, numeric_encode, raw_decode, raw_encode, rle_decode,
    rle_encode, ts1_decode, ts1_encode,
};

/// Encodes a transformed byte stream with the requested primary codec.
///
/// `lz_mode` is only consulted for [`CodecId::Lz`] and defaults to [`LzMode::Fast`];
/// [`CodecId::TimeSeries`] picks the smallest TS1 layout.
pub fn encode_codec(id: CodecId, lz_mode: Option<LzMode>, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => Ok(raw_encode(input)),
        CodecId::Rle => Ok(rle_encode(input)),
        CodecId::Lz => Ok(lz_encode(input, lz_mode.unwrap_or(LzMode::Fast))),
        CodecId::Numeric => numeric_encode(input),
        CodecId::TimeSeries => ts1_encode(input),
    }
}

/// Decodes a primary codec stream to exactly `expected_size` bytes.
pub fn decode_codec(id: CodecId, input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => raw_decode(input, expected_size),
        CodecId::Rle => rle_decode(input, expected_size),
        CodecId::Lz => lz_decode(input, expected_size),
        CodecId::Numeric => numeric_decode(input, expected_size),
        CodecId::TimeSeries => ts1_decode(input, expected_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

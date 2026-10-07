//! `EntropyCodecId` → entropy coder dispatch.

use std::borrow::Cow;

use ace_core::{AceError, AceResult, EntropyCodecId};

use crate::{
    huffman_decode, huffman_encode, rans4x_decode, rans4x_encode, rans_decode, rans_encode,
};

/// Encodes a primary-codec byte stream; returns `(entropy metadata, payload)`.
pub fn encode_entropy(id: EntropyCodecId, input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    match id {
        EntropyCodecId::None => Ok((Vec::new(), input.to_vec())),
        EntropyCodecId::Huffman => huffman_encode(input),
        EntropyCodecId::Rans => rans_encode(input),
        EntropyCodecId::Rans4x => rans4x_encode(input),
    }
}

/// Decodes an entropy-coded stream into exactly `expected_size` primary-codec bytes.
pub fn decode_entropy(
    id: EntropyCodecId,
    metadata: &[u8],
    input: &[u8],
    expected_size: usize,
) -> AceResult<Vec<u8>> {
    decode_entropy_cow(id, metadata, input, expected_size).map(Cow::into_owned)
}

/// Like [`decode_entropy`], but borrows the input for [`EntropyCodecId::None`] instead of
/// copying it (the decode hot path then copies a RAW block once instead of twice).
pub fn decode_entropy_cow<'a>(
    id: EntropyCodecId,
    metadata: &[u8],
    input: &'a [u8],
    expected_size: usize,
) -> AceResult<Cow<'a, [u8]>> {
    match id {
        EntropyCodecId::None => {
            if input.len() != expected_size {
                return Err(AceError::Malformed("raw entropy stream length mismatch"));
            }
            Ok(Cow::Borrowed(input))
        }
        EntropyCodecId::Huffman => huffman_decode(metadata, input, expected_size).map(Cow::Owned),
        EntropyCodecId::Rans => rans_decode(metadata, input, expected_size).map(Cow::Owned),
        EntropyCodecId::Rans4x => rans4x_decode(metadata, input, expected_size).map(Cow::Owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

//! Entropy coders used as the final stage of ACE 0.3 physical compression plans.

mod huffman;
mod rans;
mod rans4x;

pub use huffman::*;
pub use rans::*;
pub use rans4x::*;

use ace_core::{AceResult, EntropyCodecId};

/// Encodes a primary codec byte stream with the requested entropy coder.
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
    match id {
        EntropyCodecId::None => {
            if input.len() != expected_size {
                return Err(ace_core::AceError::Malformed(
                    "raw entropy stream length mismatch",
                ));
            }
            Ok(input.to_vec())
        }
        EntropyCodecId::Huffman => huffman_decode(metadata, input, expected_size),
        EntropyCodecId::Rans => rans_decode(metadata, input, expected_size),
        EntropyCodecId::Rans4x => rans4x_decode(metadata, input, expected_size),
    }
}

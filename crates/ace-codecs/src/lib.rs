//! Primary structural byte codecs used by ACE 0.2.1.
//! 
mod lz;
mod raw;
mod rle;

pub use lz::*;
pub use raw::*;
pub use rle::*;

use ace_core::{AceResult, CodecId, LzMode};

/// Encodes a transformed byte stream with the requested primary codec.
pub fn encode_codec(id: CodecId, lz_mode: Option<LzMode>, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => Ok(raw_encode(input)),
        CodecId::Rle => Ok(rle_encode(input)),
        CodecId::Lz => Ok(lz_encode(input, lz_mode.unwrap_or(LzMode::Fast))),
    }
}

/// Decodes a primary codec stream to its expected transformed byte length.
pub fn decode_codec(id: CodecId, input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => raw_decode(input, expected_size),
        CodecId::Rle => rle_decode(input, expected_size),
        CodecId::Lz => lz_decode(input, expected_size),
    }
}

//! Canonical Huffman encoder.

use ace_core::AceResult;

use super::bit_io::BitWriter;
use super::canonical::canonical_codes;
use super::code_lengths::build_code_lengths;

/// Encodes bytes with canonical Huffman coding; returns `(256-byte length table, payload)`.
pub fn huffman_encode(input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    if input.is_empty() {
        return Ok((vec![0u8; 256], Vec::new()));
    }
    let lengths = build_code_lengths(input)?;
    let codes = canonical_codes(&lengths)?;
    let mut writer = BitWriter::with_capacity(input.len());
    for &byte in input {
        let (code, length) = codes[byte as usize];
        writer.write(code, length);
    }
    Ok((lengths.to_vec(), writer.finish()))
}

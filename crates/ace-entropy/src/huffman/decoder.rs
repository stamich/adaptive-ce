//! Table-driven canonical Huffman decoder.

use ace_core::{AceError, AceResult};

use super::bit_io::BitReader;
use super::canonical::{DecodeTables, LOOKUP_BITS, MAX_CODE_LEN};

/// Decodes a canonical Huffman payload into exactly `expected_size` bytes.
///
/// Fast path: one table lookup resolves any code of up to `LOOKUP_BITS` (11) bits. Longer codes
/// and the last few bits of the stream fall back to the per-length canonical walk, which also
/// produces the error results (truncated stream, invalid code), so the observable behaviour is
/// identical to a pure bit-by-bit decoder.
pub fn huffman_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let lengths: &[u8; 256] = metadata
        .try_into()
        .map_err(|_| AceError::InvalidHuffman("length table must contain 256 bytes"))?;
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let tables = DecodeTables::new(lengths)?;
    let mut reader = BitReader::new(input);
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    while out.len() < expected_size {
        match tables.lookup_fast(reader.peek(LOOKUP_BITS)) {
            Some((symbol, length)) if length as usize <= reader.remaining() => {
                reader.skip(length);
                out.push(symbol);
            }
            _ => out.push(decode_slow(&tables, &mut reader)?),
        }
    }
    Ok(out)
}

/// Decodes one symbol bit by bit using the per-length canonical tables.
fn decode_slow(tables: &DecodeTables, reader: &mut BitReader<'_>) -> AceResult<u8> {
    let mut code = 0u32;
    for length in 1..=MAX_CODE_LEN {
        let bit = reader
            .read_bit()
            .ok_or(AceError::InvalidHuffman("truncated bitstream"))?;
        code = (code << 1) | bit;
        if let Some(symbol) = tables.lookup_canonical(code, length) {
            return Ok(symbol);
        }
    }
    Err(AceError::InvalidHuffman("no matching canonical code"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huffman_encode;

    /// Round-trips skewed, uniform, tiny and single-symbol inputs (fast and slow paths).
    #[test]
    fn roundtrip_varied_inputs() {
        let skew: Vec<u8> = (0..50_000u32)
            .map(|i| match i % 17 {
                0..=9 => 1,
                10..=13 => 2,
                14 | 15 => 3,
                _ => (i % 251) as u8,
            })
            .collect();
        let uniform: Vec<u8> = (0..8192u32).map(|i| (i * 131 % 256) as u8).collect();
        let mut mixed = vec![b'a'; 1000];
        mixed.extend((0u8..=255).cycle().take(4096));
        for data in [skew, uniform, mixed, vec![7u8], vec![1, 2], vec![9; 300]] {
            let (metadata, payload) = huffman_encode(&data).unwrap();
            assert_eq!(
                huffman_decode(&metadata, &payload, data.len()).unwrap(),
                data
            );
        }
    }

    /// Codes longer than the lookup width use the canonical fallback.
    #[test]
    fn long_codes_use_slow_path() {
        // Fibonacci-like frequencies force code lengths well above LOOKUP_BITS.
        let mut data = Vec::new();
        let (mut a, mut b) = (1usize, 1usize);
        for symbol in 0..20u8 {
            data.extend(std::iter::repeat(symbol).take(a));
            (a, b) = (b, a + b);
        }
        let (metadata, payload) = huffman_encode(&data).unwrap();
        assert!(metadata.iter().any(|&length| length > LOOKUP_BITS));
        assert_eq!(
            huffman_decode(&metadata, &payload, data.len()).unwrap(),
            data
        );
    }

    /// Truncated payloads, bad metadata and empty trees are rejected.
    #[test]
    fn malformed_input_is_rejected() {
        let data: Vec<u8> = (0..4096u32).map(|i| (i % 13) as u8).collect();
        let (metadata, payload) = huffman_encode(&data).unwrap();
        assert!(huffman_decode(&metadata, &payload[..payload.len() / 2], data.len()).is_err());
        assert!(huffman_decode(&metadata[..255], &payload, data.len()).is_err());
        assert!(huffman_decode(&[0u8; 256], &payload, data.len()).is_err());
        assert!(huffman_decode(&metadata, &[], 0).unwrap().is_empty());
    }
}

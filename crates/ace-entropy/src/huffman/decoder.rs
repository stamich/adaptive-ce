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
///
/// # Errors
///
/// [`AceError::InvalidHuffman`] for a length table that is not exactly 256 bytes or not a
/// valid canonical code, an invalid code, a stream that ends before `expected_size` symbols,
/// more than the zero padding of the last byte left after them, or a non-empty payload for an
/// empty output.
pub fn huffman_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let lengths: &[u8; 256] = metadata
        .try_into()
        .map_err(|_| AceError::InvalidHuffman("length table must contain 256 bytes"))?;
    if expected_size == 0 {
        // The encoder writes no bits for an empty output.
        return if input.is_empty() {
            Ok(Vec::new())
        } else {
            Err(AceError::InvalidHuffman("payload for an empty output"))
        };
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
    // Only the zero padding of the last byte may remain (the encoder pads with zero bits).
    let padding = reader.remaining();
    if padding >= 8 || (padding > 0 && reader.peek(padding as u8) != 0) {
        return Err(AceError::InvalidHuffman("trailing Huffman payload bits"));
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

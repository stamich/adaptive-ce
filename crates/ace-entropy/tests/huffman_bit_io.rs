//! Tests of `ace_entropy` `huffman_bit_io` (formerly the inline unit tests of `src`).

use ace_entropy::{MsbBitReader as BitReader, MsbBitWriter as BitWriter};

/// The accumulator writer matches a bit-at-a-time reference and the reader reads it back.
#[test]
fn writer_matches_reference_and_reader_roundtrips() {
    let codes: Vec<(u32, u8)> = (1..200u32)
        .map(|i| {
            (
                i.wrapping_mul(2_654_435_761) >> (32 - (i % 32 + 1)),
                (i % 32 + 1) as u8,
            )
        })
        .collect();
    let mut writer = BitWriter::default();
    let mut reference: Vec<u8> = Vec::new();
    let (mut current, mut used) = (0u8, 0u8);
    for &(code, length) in &codes {
        writer.write(code, length);
        for shift in (0..length).rev() {
            current = (current << 1) | ((code >> shift) & 1) as u8;
            used += 1;
            if used == 8 {
                reference.push(current);
                current = 0;
                used = 0;
            }
        }
    }
    if used != 0 {
        reference.push(current << (8 - used));
    }
    let bytes = writer.finish();
    assert_eq!(bytes, reference);

    let mut reader = BitReader::new(&bytes);
    for &(code, length) in &codes {
        let mut value = 0u32;
        for _ in 0..length {
            value = (value << 1) | reader.read_bit().unwrap();
        }
        assert_eq!(value, code);
    }
    assert!(
        reader.read_bit().is_none() || reader.remaining() < 8,
        "only padding bits remain"
    );
    let mut reader = BitReader::new(&bytes);
    assert_eq!(
        reader.peek(11),
        u32::from_be_bytes([bytes[0], bytes[1], 0, 0]) >> 21
    );
    reader.skip(3);
    assert_eq!(reader.remaining(), bytes.len() * 8 - 3);
    assert_eq!(
        reader.peek(16),
        (u32::from_be_bytes([bytes[0], bytes[1], bytes[2], 0]) << 3) >> 16
    );
}

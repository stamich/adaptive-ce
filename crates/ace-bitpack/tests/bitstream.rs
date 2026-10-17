//! Tests of `ace_bitpack` `bitstream` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// Mask with the low `width` bits set (`width` in `0..=64`).
fn low_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Reference one-bit-at-a-time writer used only to prove the fast writer is identical.
fn reference_write_bits(out: &mut [u8], bit_pos: usize, value: u64, width: u8) {
    for bit in 0..width as usize {
        if (value >> bit) & 1 != 0 {
            let position = bit_pos + bit;
            out[position / 8] |= 1u8 << (position % 8);
        }
    }
}

/// Reference one-bit-at-a-time reader used only to prove the fast reader is identical.
fn reference_read_bits(input: &[u8], bit_pos: usize, width: u8) -> u64 {
    (0..width as usize).fold(0u64, |value, bit| {
        let position = bit_pos + bit;
        value | ((((input[position / 8] >> (position % 8)) & 1) as u64) << bit)
    })
}

/// The fragment bit I/O must produce exactly the bytes of the bit-by-bit reference.
#[test]
fn fast_bit_io_matches_reference_for_all_widths_and_offsets() {
    let mut state = 0x1234_5678_9abc_def0u64;
    for width in 1u8..=64 {
        for offset in 0usize..16 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            let value = state & mask;
            let (mut fast, mut slow) = (vec![0u8; 24], vec![0u8; 24]);
            write_bits(&mut fast, offset, value, width);
            reference_write_bits(&mut slow, offset, value, width);
            assert_eq!(fast, slow, "writer width={width} offset={offset}");
            assert_eq!(read_bits_validated(&fast, offset, width), value);
            let tight = &fast[..(offset + width as usize).div_ceil(8)];
            assert_eq!(
                read_bits_validated(tight, offset, width),
                value,
                "tail fallback"
            );
            assert_eq!(reference_read_bits(&fast, offset, width), value);
        }
    }
}

/// The sequential writer produces the same bits as the positional writer, and the reader
/// returns every value back; reads past the declared length fail.
#[test]
fn sequential_writer_matches_positional_writer() {
    let mut state = 0x0ACE_0500u64;
    let mut values = Vec::new();
    let mut writer = BitWriter::with_capacity(0);
    let mut positional = vec![0u8; 600 * 8 + 8];
    let mut position = 0usize;
    for step in 0..600u32 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let width = step % 65;
        let value = state & low_mask(width);
        writer.write_bits(value, width);
        if width > 0 {
            write_bits(&mut positional, position, value, width as u8);
        }
        position += width as usize;
        values.push((value, width));
    }
    let (bytes, bit_len) = writer.finish();
    assert_eq!(bit_len as usize, position);
    assert_eq!(bytes, positional[..position.div_ceil(8)]);
    let mut reader = BitReader::new(&bytes, bit_len).unwrap();
    for (value, width) in values {
        assert_eq!(reader.read_bits(width), Some(value), "width {width}");
    }
    assert_eq!(reader.remaining(), 0);
    assert_eq!(reader.read_bit(), None);
}

/// Length mismatches and over-long reads are rejected.
#[test]
fn reader_validates_lengths() {
    assert!(BitReader::new(&[0u8; 2], 17).is_err());
    assert!(BitReader::new(&[0u8; 3], 8).is_err());
    let mut reader = BitReader::new(&[0xFF], 5).unwrap();
    assert_eq!(reader.read_bits(6), None);
    assert_eq!(reader.read_bits(5), Some(0b11111));
    assert_eq!(reader.read_bits(65), None);
    let (empty, bits) = BitWriter::default().finish();
    assert_eq!((empty.len(), bits), (0, 0));
}

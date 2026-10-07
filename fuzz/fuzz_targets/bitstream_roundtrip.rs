#![no_main]

use ace_bitpack::{BitReader, BitWriter};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // 1. Arbitrary (width, value) pairs written sequentially must be read back exactly.
    let mut writer = BitWriter::default();
    let mut expected = Vec::new();
    for chunk in data.chunks_exact(9) {
        let width = u32::from(chunk[0] % 65);
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&chunk[1..]);
        let value = u64::from_le_bytes(raw);
        let masked = if width == 64 { value } else { value & ((1u64 << width) - 1) };
        writer.write_bits(value, width);
        expected.push((masked, width));
    }
    let (bytes, bit_len) = writer.finish();
    let mut reader = BitReader::new(&bytes, bit_len).expect("own stream is well-formed");
    for (value, width) in expected {
        assert_eq!(reader.read_bits(width), Some(value));
    }
    assert_eq!(reader.remaining(), 0);

    // 2. Arbitrary bytes with an arbitrary declared length never panic.
    if let Some((&declared, rest)) = data.split_first() {
        if let Ok(mut reader) = BitReader::new(rest, u64::from(declared)) {
            while reader.read_bits(u32::from(declared % 64) + 1).is_some() {}
        }
    }
});

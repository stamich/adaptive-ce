//! Tests of `ace_codecs` `rle` (formerly the inline unit tests of `src`).

use ace_codecs::*;

/// Verifies literals, short runs and long runs in the same stream.
#[test]
fn mixed_roundtrip() {
    let mut d = b"abc".to_vec();
    d.extend(vec![7u8; 500]);
    d.extend_from_slice(b"xyzxyz");
    assert_eq!(rle_decode(&rle_encode(&d), d.len()).unwrap(), d);
}

/// Verifies the literal-packet boundary around the 128-byte wire-format limit.
#[test]
fn literal_packet_boundary_roundtrip() {
    for size in [126usize, 127, 128, 129, 130, 255, 256, 257] {
        let data = (0..size)
            .map(|i| ((i * 37 + i / 3) % 251) as u8)
            .collect::<Vec<_>>();
        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, data.len()).unwrap();
        assert_eq!(
            decoded, data,
            "RLE boundary roundtrip failed for size {size}"
        );
    }
}

/// Reproduces the ACE 0.3 bug where many three-byte runs could overflow a literal packet.
#[test]
fn repeated_short_runs_do_not_overflow_literal_packet() {
    let mut data = Vec::new();
    for value in 0u8..100 {
        data.extend_from_slice(&[value, value, value]);
    }
    let encoded = rle_encode(&data);
    let decoded = rle_decode(&encoded, data.len()).unwrap();
    assert_eq!(decoded, data);
}

/// Exercises deterministic pseudo-random payloads across a broad range of lengths.
#[test]
fn deterministic_fuzz_style_roundtrip() {
    let mut state = 0x1234_5678u32;
    for size in [0usize, 1, 2, 3, 4, 127, 128, 129, 1024, 4097, 65535] {
        let mut data = Vec::with_capacity(size);
        for _ in 0..size {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            data.push((state >> 24) as u8);
        }
        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, data.len()).unwrap();
        assert_eq!(
            decoded, data,
            "RLE deterministic fuzz-style roundtrip failed for size {size}"
        );
    }
}

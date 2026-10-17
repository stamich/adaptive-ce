//! Tests of `ace_entropy` `huffman_decoder` (formerly the inline unit tests of `src`).

use ace_entropy::huffman_encode;
use ace_entropy::*;

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
        data.extend(std::iter::repeat_n(symbol, a));
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

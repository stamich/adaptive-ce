//! Chimp / Chimp128 (evaluation codecs of the `chimp-evaluation` family): exact round trips,
//! sensible sizes and rejection of damaged streams.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_bench::chimp::{chimp128_decode, chimp128_encode, chimp_decode, chimp_encode};

/// Round-trips `values` through both codecs and returns the two bit lengths.
fn round_trip(values: &[u64]) -> (u64, u64) {
    let (stream, bits) = chimp_encode(values);
    assert_eq!(chimp_decode(&stream, bits, values.len()).unwrap(), values);
    let (stream128, bits128) = chimp128_encode(values);
    assert_eq!(
        chimp128_decode(&stream128, bits128, values.len()).unwrap(),
        values
    );
    (bits, bits128)
}

/// Deterministic xorshift64 draws.
fn draws(seed: u64) -> impl Iterator<Item = u64> {
    let mut state = seed;
    std::iter::repeat_with(move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    })
}

#[test]
fn empty_single_and_constant() {
    assert_eq!(round_trip(&[]), (0, 0));
    assert_eq!(round_trip(&[0x4009_21fb_5444_2d18]), (64, 64));
    let (chimp, chimp128) = round_trip(&[7; 1000]);
    assert_eq!(chimp, 64 + 999 * 2);
    assert_eq!(chimp128, 64 + 999 * 9);
}

#[test]
fn arbitrary_bit_patterns_round_trip() {
    for seed in [1u64, 0xdead_beef, 0x1234_5678_9abc_def1] {
        let random: Vec<u64> = draws(seed).take(2_000).collect();
        round_trip(&random);
    }
    let special = [
        0u64,
        u64::MAX,
        1,
        1 << 63,
        f64::NAN.to_bits(),
        (-0.0f64).to_bits(),
        f64::INFINITY.to_bits(),
        0x0000_0000_0000_00ff,
        0xff00_0000_0000_0000,
    ];
    let mixed: Vec<u64> = (0..3_000).map(|i| special[(i * 7 + i / 5) % 9]).collect();
    round_trip(&mixed);
}

#[test]
fn smooth_series_and_repeats_in_a_window() {
    let smooth: Vec<u64> = (0..5_000)
        .map(|i| (20.0 + f64::from(i) * 0.01).to_bits())
        .collect();
    let (chimp, _) = round_trip(&smooth);
    assert!(chimp < 5_000 * 64, "{chimp}");
    // A value that recurs every 50 samples is found through the Chimp128 window.
    let cycle: Vec<u64> = (0..5_000u64)
        .map(|i| (i % 50).wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .collect();
    let (chimp, chimp128) = round_trip(&cycle);
    assert!(chimp128 < chimp, "{chimp128} vs {chimp}");
}

#[test]
fn damaged_streams_are_rejected_or_harmless() {
    let values: Vec<u64> = draws(9).take(300).collect();
    let (stream, bits) = chimp_encode(&values);
    assert!(chimp_decode(&stream, bits, values.len() + 1).is_err());
    assert!(chimp_decode(&stream[..stream.len() / 2], bits, values.len()).is_err());
    let (stream128, bits128) = chimp128_encode(&values);
    assert!(chimp128_decode(&stream128, bits128, values.len() + 1).is_err());
    for byte in (0..stream.len()).step_by(7) {
        let mut damaged = stream.clone();
        damaged[byte] ^= 0xa5;
        let _ = chimp_decode(&damaged, bits, values.len());
        let mut damaged = stream128.clone();
        damaged[byte % stream128.len()] ^= 0xa5;
        let _ = chimp128_decode(&damaged, bits128, values.len());
    }
}

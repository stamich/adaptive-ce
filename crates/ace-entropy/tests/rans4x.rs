//! Tests of `ace_entropy` `rans4x` (formerly the inline unit tests of `src`).

use ace_entropy::*;

/// Verifies exact round-trip across uneven lane lengths.
#[test]
fn round_trip() {
    let data: Vec<u8> = (0u8..=255).cycle().take(100_003).collect();
    let (metadata, payload) = rans4x_encode(&data).unwrap();
    assert_eq!(
        rans4x_decode(&metadata, &payload, data.len()).unwrap(),
        data
    );
}

/// Every length around the lane boundary (including empty) restores the order of the bytes.
#[test]
fn every_small_length_round_trips() {
    for length in 0..=40usize {
        let data: Vec<u8> = (0..length)
            .map(|i| u8::try_from(i * 37 % 251).unwrap())
            .collect();
        let (metadata, payload) = rans4x_encode(&data).unwrap();
        assert_eq!(
            rans4x_decode(&metadata, &payload, length).unwrap(),
            data,
            "length {length}"
        );
    }
}

/// Asking for more bytes than were encoded is an error, never a short or padded output.
#[test]
fn a_wrong_expected_size_is_rejected() {
    let data: Vec<u8> = (0u8..=255).cycle().take(1001).collect();
    let (metadata, payload) = rans4x_encode(&data).unwrap();
    for wrong in [0usize, 1, 1000, 1002, 4004] {
        assert!(
            rans4x_decode(&metadata, &payload, wrong).is_err(),
            "{wrong}"
        );
    }
}

//! Tests of LZ with a preset dictionary (Format 1.6): the dictionary is history that precedes
//! the input, and the token stream is unchanged.

use ace_codecs::{lz_decode, lz_decode_with_dictionary, lz_encode, lz_encode_with_dictionary};
use ace_core::{AceError, LzMode};

/// Deterministic pseudo-random bytes.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

const MODES: [LzMode; 2] = [LzMode::Fast, LzMode::Balanced];

#[test]
fn round_trip_for_many_dictionaries_and_inputs() {
    let source = noise(8192, 7);
    for mode in MODES {
        for dict_len in [1usize, 3, 4, 17, 500, 4000] {
            for input_len in [0usize, 1, 3, 4, 100, 3000] {
                // Input and dictionary are windows of one source, so they share content.
                let dictionary = &source[100..100 + dict_len];
                let start = 100 + dict_len / 2;
                let input = &source[start..(start + input_len).min(source.len())];
                let encoded = lz_encode_with_dictionary(input, dictionary, mode);
                let decoded = lz_decode_with_dictionary(&encoded, dictionary, input.len()).unwrap();
                assert_eq!(decoded, input, "dict {dict_len} input {input_len} {mode:?}");
            }
        }
    }
}

#[test]
fn an_empty_dictionary_is_the_plain_codec() {
    let data = noise(2000, 3).repeat(3);
    for mode in MODES {
        assert_eq!(
            lz_encode_with_dictionary(&data, &[], mode),
            lz_encode(&data, mode)
        );
        let encoded = lz_encode(&data, mode);
        assert_eq!(
            lz_decode_with_dictionary(&encoded, &[], data.len()).unwrap(),
            lz_decode(&encoded, data.len()).unwrap()
        );
    }
}

#[test]
fn the_dictionary_makes_a_short_input_smaller() {
    let dictionary =
        b"{\"event\":\"temperature_reading\",\"unit\":\"celsius\",\"firmware\":\"release-2.7.1\"}";
    let input = b"{\"event\":\"temperature_reading\",\"unit\":\"celsius\",\"value\":21}";
    for mode in MODES {
        let plain = lz_encode(input, mode);
        let with = lz_encode_with_dictionary(input, dictionary, mode);
        assert!(
            with.len() < plain.len(),
            "{} !< {}",
            with.len(),
            plain.len()
        );
        assert_eq!(
            lz_decode_with_dictionary(&with, dictionary, input.len()).unwrap(),
            input
        );
    }
}

#[test]
fn the_output_is_deterministic() {
    let dictionary = noise(3000, 11);
    let input = [&dictionary[500..2500], &noise(500, 12)[..]].concat();
    for mode in MODES {
        assert_eq!(
            lz_encode_with_dictionary(&input, &dictionary, mode),
            lz_encode_with_dictionary(&input, &dictionary, mode)
        );
    }
}

#[test]
fn a_match_may_run_from_the_dictionary_into_the_output() {
    // The input continues the dictionary exactly, so one reference covers the seam.
    let source = noise(600, 21);
    let (dictionary, input) = source.split_at(300);
    let mut shifted = dictionary[200..].to_vec();
    shifted.extend_from_slice(input);
    let encoded = lz_encode_with_dictionary(&shifted, dictionary, LzMode::Balanced);
    assert_eq!(
        lz_decode_with_dictionary(&encoded, dictionary, shifted.len()).unwrap(),
        shifted
    );
}

#[test]
fn a_reference_before_the_dictionary_is_invalid() {
    // One match token: length 4, distance 5, with a three-byte dictionary and no output yet.
    let stream = [0x80, 5, 0];
    assert!(matches!(
        lz_decode_with_dictionary(&stream, b"abc", 4),
        Err(AceError::InvalidLzReference)
    ));
    // Distance 3 reaches exactly the first dictionary byte: valid, and copies "abca".
    let stream = [0x80, 3, 0];
    assert_eq!(
        lz_decode_with_dictionary(&stream, b"abc", 4).unwrap(),
        b"abca"
    );
}

#[test]
fn a_stream_that_needs_the_dictionary_fails_without_it() {
    let dictionary = noise(1000, 5);
    let input = dictionary[100..900].to_vec();
    let encoded = lz_encode_with_dictionary(&input, &dictionary, LzMode::Balanced);
    // The first token is a reference into the dictionary: without it, there is no history.
    assert!(matches!(
        lz_decode(&encoded, input.len()),
        Err(AceError::InvalidLzReference)
    ));
    // A different dictionary decodes to different bytes or fails, never to the input.
    let other = noise(1000, 6);
    if let Ok(bytes) = lz_decode_with_dictionary(&encoded, &other, input.len()) {
        assert_ne!(bytes, input);
    }
}

#[test]
fn size_checks_count_only_the_output() {
    let dictionary = b"0123456789abcdef";
    let input = b"0123456789abcdef!";
    let encoded = lz_encode_with_dictionary(input, dictionary, LzMode::Fast);
    assert!(lz_decode_with_dictionary(&encoded, dictionary, input.len() - 1).is_err());
    assert!(lz_decode_with_dictionary(&encoded, dictionary, input.len() + 1).is_err());
    assert_eq!(
        lz_decode_with_dictionary(&encoded, dictionary, input.len()).unwrap(),
        input
    );
    // A huge declared size does not allocate the declared amount up front.
    assert!(lz_decode_with_dictionary(&encoded, dictionary, usize::MAX).is_err());
}

#[test]
fn references_never_exceed_the_window() {
    // A 65 535-byte dictionary: the first dictionary byte is out of reach of the first output
    // byte (distance 65 536), so the encoder must not use it, and the decoder must reject it.
    let dictionary = noise(65_535, 31);
    let input = [&dictionary[..64], &noise(64, 32)[..]].concat();
    let encoded = lz_encode_with_dictionary(&input, &dictionary, LzMode::Balanced);
    assert_eq!(
        lz_decode_with_dictionary(&encoded, &dictionary, input.len()).unwrap(),
        input
    );
    let forged = [0x80, 0, 0]; // distance 0
    assert!(lz_decode_with_dictionary(&forged, &dictionary, 4).is_err());
}

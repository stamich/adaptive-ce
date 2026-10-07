//! Unit tests of the TS1 codec (header, Gorilla, RunDelta, edge cases).

use super::*;

/// A valid header round-trips through serialization and inspection.
#[test]
fn header_roundtrip_and_validation() {
    let info = TimeSeriesPayloadInfo {
        mode: TimeSeriesMode::RunDelta,
        lane_bytes: 4,
        flags: 0,
        value_count: 3,
        tail_len: 2,
        first_bits: 0xDEAD_BEEF,
        stream_bits: 13,
    };
    let payload = serialize_payload(&info, &[0xAB, 0x1F], &[7, 9]).unwrap();
    assert_eq!(payload.len(), TIME_SERIES_HEADER_SIZE + 2 + 2);
    assert_eq!(ts1_inspect(&payload).unwrap(), info);
    assert_eq!(info.decoded_len().unwrap(), 14);
}

/// Every single-field corruption of the header is rejected by `ts1_inspect`.
#[test]
fn header_corruptions_are_rejected() {
    let info = TimeSeriesPayloadInfo {
        mode: TimeSeriesMode::GorillaF32,
        lane_bytes: 4,
        flags: FLAG_WINDOW_REUSE,
        value_count: 2,
        tail_len: 1,
        first_bits: 0x3F80_0000,
        stream_bits: 9,
    };
    let valid = serialize_payload(&info, &[1, 0], &[5]).unwrap();
    let corruptions: [(usize, u8); 11] = [
        (0, b'X'),  // magic
        (4, 2),     // version
        (5, 9),     // mode
        (6, 8),     // lane width for f32
        (7, 0x82),  // unknown flag
        (12, 4),    // tail as long as a lane
        (13, 1),    // reserved byte
        (20, 1),    // first value above 32 bits
        (24, 200),  // stream length mismatch
        (8, 0),     // empty series with data
        (25, 0xFF), // huge stream length
    ];
    for (offset, value) in corruptions {
        let mut payload = valid.clone();
        payload[offset] = value;
        assert!(ts1_inspect(&payload).is_err(), "offset {offset}");
    }
    assert!(ts1_inspect(&valid[..TIME_SERIES_HEADER_SIZE - 1]).is_err());
    let mut longer = valid.clone();
    longer.push(0);
    assert!(ts1_inspect(&longer).is_err());
}

/// Mode ids, lane widths and labels are stable.
#[test]
fn modes_are_stable() {
    for mode in TimeSeriesMode::ALL {
        assert_eq!(TimeSeriesMode::try_from(mode as u8).unwrap(), mode);
    }
    assert!(TimeSeriesMode::try_from(0).is_err());
    assert!(TimeSeriesMode::try_from(4).is_err());
    assert!(TimeSeriesMode::RunDelta.accepts_lane_bytes(2));
    assert!(!TimeSeriesMode::GorillaF64.accepts_lane_bytes(4));
    assert_eq!(TimeSeriesMode::GorillaF64.label(), "gorilla_f64");
}

/// Bytes of a slice of f64 values.
fn f64_bytes(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Bytes of a slice of f32 values.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Encodes with `mode`, checks the exact size estimate and decodes bit-exactly.
fn roundtrip(input: &[u8], mode: TimeSeriesMode) -> Vec<u8> {
    let encoded = ts1_encode_with(input, mode).unwrap();
    assert_eq!(
        ts1_encoded_len(input, mode),
        encoded.len(),
        "{mode:?} size estimate"
    );
    assert_eq!(
        ts1_decode(&encoded, input.len()).unwrap(),
        input,
        "{mode:?} roundtrip"
    );
    encoded
}

/// IEEE special values survive bit-exactly (NaN payloads, signed zeros, infinities,
/// subnormals, extremes) for both widths.
#[test]
fn gorilla_is_bit_exact_for_special_values() {
    let specials64 = [
        0.0,
        -0.0,
        f64::NAN,
        f64::from_bits(0x7FF0_0000_0000_0001),
        f64::from_bits(0xFFF8_0000_DEAD_BEEF),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE / 3.0,
        f64::MAX,
        f64::MIN,
        1.0,
        1.0,
    ];
    roundtrip(&f64_bytes(&specials64), TimeSeriesMode::GorillaF64);
    let specials32 = [
        0.0f32,
        -0.0,
        f32::NAN,
        f32::from_bits(0x7F80_0001),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE / 3.0,
        f32::MAX,
        f32::MIN,
    ];
    roundtrip(&f32_bytes(&specials32), TimeSeriesMode::GorillaF32);
}

/// Constant series cost one bit per value; slowly changing series stay well below 64.
#[test]
fn gorilla_compresses_constant_and_smooth_series() {
    let constant = f64_bytes(&[21.5; 4096]);
    let encoded = roundtrip(&constant, TimeSeriesMode::GorillaF64);
    let info = ts1_inspect(&encoded).unwrap();
    assert_eq!(info.stream_bits, 4095);
    let mut value = 20.0f64;
    let smooth: Vec<f64> = (0..4096)
        .map(|i| {
            value += ((i % 7) as f64 - 3.0) * 0.001;
            value
        })
        .collect();
    let encoded = roundtrip(&f64_bytes(&smooth), TimeSeriesMode::GorillaF64);
    assert!(ts1_inspect(&encoded).unwrap().bits_per_value() < 56.0);
}

/// Tails, empty inputs and single values are handled for both Gorilla widths.
#[test]
fn gorilla_handles_tails_and_short_inputs() {
    for len in [0usize, 1, 3, 4, 7, 8, 9, 15, 16, 17, 33] {
        let input: Vec<u8> = (0..len as u8).map(|b| b.wrapping_mul(37)).collect();
        roundtrip(&input, TimeSeriesMode::GorillaF64);
        roundtrip(&input, TimeSeriesMode::GorillaF32);
    }
}

/// Window reuse is optional for the decoder: a stream without reuse decodes too, and a
/// stream that reuses an undefined window is rejected.
#[test]
fn gorilla_decoder_honours_window_flag() {
    let values: Vec<f64> = (0..64).map(|i| 1.0 + i as f64 * 0.25).collect();
    let input = f64_bytes(&values);
    let mut writer = ace_bitpack::BitWriter::default();
    gorilla::encode_xors(
        gorilla::GORILLA_F64,
        input.chunks_exact(8).map(ace_core::read_lane::<8>),
        false,
        &mut writer,
    );
    let (stream, bits) = writer.finish();
    let info = TimeSeriesPayloadInfo {
        mode: TimeSeriesMode::GorillaF64,
        lane_bytes: 8,
        flags: 0,
        value_count: 64,
        tail_len: 0,
        first_bits: values[0].to_bits(),
        stream_bits: bits,
    };
    let payload = serialize_payload(&info, &stream, &[]).unwrap();
    assert_eq!(ts1_decode(&payload, input.len()).unwrap(), input);

    // '1' (non-zero XOR), '0' (reuse) as the very first control bits: no window yet.
    let bad = TimeSeriesPayloadInfo {
        flags: FLAG_WINDOW_REUSE,
        value_count: 2,
        stream_bits: 2,
        ..info
    };
    let payload = serialize_payload(&bad, &[0b01], &[]).unwrap();
    assert!(ts1_decode(&payload, 16).is_err());
}

/// Truncated streams, trailing bits and invalid windows are errors, never panics.
#[test]
fn gorilla_rejects_corrupt_streams() {
    let values: Vec<f64> = (0..200).map(|i| (i as f64).sqrt()).collect();
    let input = f64_bytes(&values);
    let valid = ts1_encode_with(&input, TimeSeriesMode::GorillaF64).unwrap();
    assert!(ts1_decode(&valid, input.len() - 8).is_err());
    for position in TIME_SERIES_HEADER_SIZE..valid.len() {
        let mut corrupt = valid.clone();
        corrupt[position] ^= 0x5A;
        if let Ok(decoded) = ts1_decode(&corrupt, input.len()) {
            assert_eq!(decoded.len(), input.len());
        }
    }
}

#![no_main]

use ace_codecs::{
    ts1_decode, ts1_inspect, ts1_serialize_payload, TimeSeriesMode, TimeSeriesPayloadInfo,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The fuzzer controls the ALP *stream*; the header is built around it so the stream decoder
    // (exponents, width, base, exception list, packed offsets) is reached on every input.
    let Some((&control, stream)) = data.split_first() else {
        return;
    };
    let lane_bytes: u8 = if control & 1 == 0 { 8 } else { 4 };
    let value_count = 1 + usize::from(control >> 1) * 8 + stream.len() % 7;
    let info = TimeSeriesPayloadInfo {
        mode: TimeSeriesMode::Alp,
        lane_bytes,
        flags: 0,
        value_count,
        tail_len: 0,
        first_bits: 0,
        stream_bits: stream.len() as u64 * 8,
    };
    let Ok(payload) = ts1_serialize_payload(&info, stream, &[]) else {
        return;
    };
    // A stream that decodes must give exactly the declared number of bytes.
    let expected = value_count * usize::from(lane_bytes);
    assert!(ts1_inspect(&payload).is_ok());
    if let Ok(decoded) = ts1_decode(&payload, expected) {
        assert_eq!(decoded.len(), expected);
    }
});

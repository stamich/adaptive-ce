#![no_main]

use ace_codecs::{ts1_decode, ts1_encode_with, ts1_encoded_len, TimeSeriesLayout};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes treated as source data: every TS1 layout must be lossless (bit-exact,
    // including NaN payloads) and the exact size counter must match the encoder.
    for layout in TimeSeriesLayout::ALL {
        let encoded = ts1_encode_with(data, layout).expect("valid layout");
        assert_eq!(ts1_encoded_len(data, layout).expect("valid layout"), encoded.len());
        assert_eq!(ts1_decode(&encoded, data.len()).expect("own payload decodes"), data);
    }
});

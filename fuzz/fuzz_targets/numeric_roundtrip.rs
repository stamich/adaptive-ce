#![no_main]

use ace_codecs::{estimate_numeric, numeric_decode, numeric_encode, numeric_inspect};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // 1. Arbitrary bytes treated as a NUM1 payload: inspect/decode must reject, never panic.
    let _ = numeric_inspect(data);
    let _ = numeric_decode(data, data.len().saturating_mul(8).min(1 << 20));

    // 2. Arbitrary bytes treated as source data: encode -> decode must be lossless for every
    //    lane width (the analyzer picks u16/u32/u64 from the content) and the exact estimator
    //    must predict the encoded length byte for byte.
    if let Ok(encoded) = numeric_encode(data) {
        let decoded = numeric_decode(&encoded, data.len()).expect("own payload must decode");
        assert_eq!(decoded, data);
        if let Some(estimate) = estimate_numeric(data) {
            assert_eq!(estimate.encoded_bytes, encoded.len());
        }
    }
});

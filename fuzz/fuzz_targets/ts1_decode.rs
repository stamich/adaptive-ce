#![no_main]

use ace_codecs::{ts1_decode, ts1_inspect};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary TS1 payloads must be rejected or decoded to exactly the requested size, never
    // panic. The expected size is bounded so a hostile header cannot request a huge buffer.
    let _ = ts1_inspect(data);
    let expected = data.len().saturating_mul(16).min(1 << 20);
    if let Ok(decoded) = ts1_decode(data, expected) {
        assert_eq!(decoded.len(), expected);
    }
});

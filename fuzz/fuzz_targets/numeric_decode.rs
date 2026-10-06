#![no_main]

use ace_codecs::numeric_decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary numeric metadata/payload must reject safely without panic. The expected output
    // size is deliberately bounded so a malformed payload cannot request a giant allocation.
    let expected = data.len().saturating_mul(8).min(1 << 20);
    let _ = numeric_decode(data, expected);
});

#![no_main]

use ace_entropy::rans4x_decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // rANS4x metadata and lane payload validation must reject arbitrary bytes without panicking.
    let split = data.len().min(2064);
    let _ = rans4x_decode(&data[..split], &data[split..], 4096);
});

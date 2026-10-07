#![no_main]

use ace_entropy::rans_decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Scalar rANS metadata/payload parsing must reject arbitrary bytes without panicking.
    let split = data.len().min(512);
    let _ = rans_decode(&data[..split], &data[split..], 1024);
});

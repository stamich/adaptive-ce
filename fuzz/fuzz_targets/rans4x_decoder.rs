#![no_main]
use libfuzzer_sys::fuzz_target;
use ace_entropy::rans4x_decode;

/// Fuzzes ACE 0.3 rANS4x metadata and lane payload validation.
fuzz_target!(|data: &[u8]| {
    let split = data.len().min(2064);
    let metadata = &data[..split];
    let payload = &data[split..];
    let _ = rans4x_decode(metadata, payload, 4096);
});

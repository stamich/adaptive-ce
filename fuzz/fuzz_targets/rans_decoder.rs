#![no_main]
use libfuzzer_sys::fuzz_target;
use ace_entropy::rans_decode;

/// Fuzzes scalar rANS metadata/payload parsing with a bounded synthetic expected output size.
fuzz_target!(|data: &[u8]| {
    let split = data.len().min(512);
    let metadata = &data[..split];
    let payload = &data[split..];
    let _ = rans_decode(metadata, payload, 1024);
});

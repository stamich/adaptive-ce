#![no_main]

use ace_engine::AceEngine;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Malformed input is expected to return an error. The hardening invariant is that decoding
    // never panics or performs an unbounded allocation under the engine's default DecodeLimits.
    let _ = AceEngine::default_engine().decompress(data);
});

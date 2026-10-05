#![no_main]

use ace_core::DecodeLimits;
use ace_format::decode_index;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The parser must reject arbitrary bytes without panicking and must enforce entry limits
    // before allocating based on attacker-controlled counts.
    let _ = decode_index(data, &DecodeLimits::default());
});

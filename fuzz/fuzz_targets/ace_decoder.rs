#![no_main]
use libfuzzer_sys::fuzz_target;
use ace_engine::AceEngine;

/// Fuzzes the complete bounded ACE decoder; arbitrary input must return success or an error without panicking.
fuzz_target!(|data: &[u8]| {
    let _ = AceEngine::default_engine().decompress(data);
});

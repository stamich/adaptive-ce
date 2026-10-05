#![no_main]

use std::io::Cursor;

use ace_core::DecodeLimits;
use ace_engine::AceIndexedDecoder;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Opening an arbitrary indexed container must fail safely when the header/index/trailer is
    // malformed. Successfully opened random input is dropped without attempting unbounded work.
    let _ = AceIndexedDecoder::open(Cursor::new(data), DecodeLimits::default());
});

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes through the C ABI (`ace_decompress`, `ace_decompress_into`, `ace_range_*`)
    // under a 4 MiB output limit. Any status is fine except a contained panic or internal error.
    ace_ffi::fuzzing::decode(data);
});

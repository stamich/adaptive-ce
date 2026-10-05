#![no_main]

use ace_format::decode_trailer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The fixed-size trailer parser must remain total for every byte slice.
    let _ = decode_trailer(data);
});

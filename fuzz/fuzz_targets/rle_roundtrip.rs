#![no_main]

use ace_codecs::{rle_decode, rle_encode};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let encoded = rle_encode(data);
    let decoded = rle_decode(&encoded, data.len()).expect("RLE output produced by encoder must decode");
    assert_eq!(decoded, data);
});

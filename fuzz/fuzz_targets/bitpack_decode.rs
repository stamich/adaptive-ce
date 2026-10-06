#![no_main]

use ace_bitpack::{unpack_u32, unpack_u64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let width32 = data[0] % 33;
    let width64 = data[0] % 65;
    let count = (data[1] as usize).min(64);
    let payload = &data[2..];
    let _ = unpack_u32(payload, count, width32);
    let _ = unpack_u64(payload, count, width64);
});

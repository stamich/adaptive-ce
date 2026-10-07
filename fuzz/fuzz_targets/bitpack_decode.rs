#![no_main]

use ace_bitpack::unpack;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary packed streams with arbitrary widths/counts must be rejected or decoded,
    // never panic, for every lane (widths beyond the lane are part of the input space).
    if data.len() < 2 {
        return;
    }
    let count = (data[1] as usize).min(64);
    let payload = &data[2..];
    let _ = unpack::<u16>(payload, count, data[0] % 17);
    let _ = unpack::<u32>(payload, count, data[0] % 33);
    let _ = unpack::<u64>(payload, count, data[0] % 65);
});

#![no_main]

use ace_codecs::{numeric_decode, numeric_encode_with, NumericMode};
use ace_core::NumericWidth;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Builds a valid Patched FOR payload (a ramp with fuzzer-placed outliers), checks it
    // round-trips, then applies fuzzer-chosen byte mutations: the exception section (count,
    // positions, high bits) must be rejected or decoded, never panic.
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let count = 64 + usize::from(selector % 5) * 40;
    let mut values: Vec<u32> = (0..count as u32).map(|i| 1_000 + i).collect();
    let (spikes, mutations) = rest.split_at(rest.len().min(24));
    for (k, spike) in spikes.chunks_exact(3).enumerate() {
        let position = (usize::from(spike[0]) * 7 + k * 13) % count;
        values[position] =
            values[position].wrapping_add(u32::from(spike[1]) << (8 + u32::from(spike[2] % 16)));
    }
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    let Ok(payload) = numeric_encode_with(&bytes, NumericWidth::U32, NumericMode::Patched) else {
        return;
    };
    assert_eq!(numeric_decode(&payload, bytes.len()).expect("own payload decodes"), bytes);
    let mut mutated = payload;
    for pair in mutations.chunks_exact(2) {
        let at = usize::from(pair[0]) * 3 % mutated.len();
        mutated[at] ^= pair[1].max(1);
    }
    if let Ok(decoded) = numeric_decode(&mutated, bytes.len()) {
        assert_eq!(decoded.len(), bytes.len());
    }
});

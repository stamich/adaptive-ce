#![no_main]
use libfuzzer_sys::fuzz_target;
use ace_cost::{deterministic_sample_ranges, SamplePolicy};

/// Fuzzes deterministic sample-range generation and asserts every returned range stays in bounds.
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let len = ((data[0] as usize) << 12) | ((data[1] as usize) << 4) | (data[2] as usize & 0x0f);
    let policy = SamplePolicy { sample_bytes: 4096, sample_count: 3, top_k: 3 };
    for range in deterministic_sample_ranges(len, policy) {
        assert!(range.start <= range.end);
        assert!(range.end <= len);
    }
});

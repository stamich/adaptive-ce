#![no_main]

use ace_core::CompressionProfile;
use ace_cost::{deterministic_sample_ranges, SamplePolicy};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Deterministic sample-range generation must stay in bounds for every input length and
    // every profile budget.
    if data.len() < 4 {
        return;
    }
    let len = (usize::from(data[0]) << 16) | (usize::from(data[1]) << 8) | usize::from(data[2]);
    let profile = match data[3] % 3 {
        0 => CompressionProfile::Fast,
        1 => CompressionProfile::Balanced,
        _ => CompressionProfile::Dense,
    };
    for range in deterministic_sample_ranges(len, SamplePolicy::for_profile(profile)) {
        assert!(range.start <= range.end);
        assert!(range.end <= len);
    }
});

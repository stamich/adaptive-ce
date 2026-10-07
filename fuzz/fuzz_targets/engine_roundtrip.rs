#![no_main]

use ace_core::{AceConfig, CompressionProfile};
use ace_engine::AceEngine;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary source bytes must round-trip losslessly through the full planner/encoder for
    // every profile (small blocks so several planner decisions are exercised per input).
    let Some((&selector, source)) = data.split_first() else {
        return;
    };
    let profile = match selector % 3 {
        0 => CompressionProfile::Fast,
        1 => CompressionProfile::Balanced,
        _ => CompressionProfile::Dense,
    };
    let engine = AceEngine::new(AceConfig {
        profile,
        block_size: 4096,
        threads: 1,
        ..AceConfig::default()
    })
    .expect("valid fuzz configuration");
    let encoded = engine.compress(source).expect("compression of any input succeeds");
    assert_eq!(engine.decompress(&encoded).expect("own output decodes"), source);
});

//! Tests of `ace` `explain` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ace::*;

#[test]
fn the_report_names_the_pipeline_that_compress_uses() {
    let data: Vec<u8> = (0..100_000u32)
        .flat_map(|i| (i * 7).to_le_bytes())
        .collect();
    let engine = Engine::new(Config::builder().block_size(128 * 1024).build().unwrap()).unwrap();
    let report = engine.explain(&data).unwrap();
    let (_, stats) = engine.compress_with_stats(&data).unwrap();
    assert_eq!(report.len() as u64, stats.blocks);
    for r in &report {
        assert!(r.candidates.contains(&r.selected), "{r:?}");
        assert!(stats.plan_distribution.contains_key(&r.selected), "{r:?}");
    }
    assert_eq!(report[0].block_id, 0);
}

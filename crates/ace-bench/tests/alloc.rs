//! Tests of `ace_bench` `alloc` (formerly the inline unit tests of `src`).

use ace_bench::alloc::count_allocations;

#[test]
fn probe_counts_a_vector_allocation() {
    let (len, report) = count_allocations(|| {
        let buffer = std::hint::black_box(vec![0u8; 4096]);
        buffer.len()
    });
    assert_eq!(len, 4096);
    assert!(report.allocations >= 1);
    assert!(report.allocated_bytes >= 4096);
    assert!(report.peak_live_bytes >= 4096);
}

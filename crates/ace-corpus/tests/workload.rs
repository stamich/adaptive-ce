//! Tests of `ace_corpus` `workload` (formerly the inline unit tests of `src`).

use ace_corpus::*;

/// Names round-trip, aliases resolve, every generator honours the requested size.
#[test]
fn names_aliases_and_sizes() {
    for workload in Workload::ALL {
        assert_eq!(Workload::from_name(workload.name()), Some(workload));
        for bytes in [0usize, 1, 7, 4097, 70_001] {
            assert_eq!(workload.generate(bytes).len(), bytes, "{workload}");
        }
        assert_eq!(
            workload.generate(10_000),
            workload.generate(10_000),
            "{workload} deterministic"
        );
    }
    assert_eq!(
        Workload::from_name("numeric-u32"),
        Some(Workload::U32Counter)
    );
    assert_eq!(Workload::from_name("nope"), None);
}

//! Tests of `ace_corpus` `false_positive` (formerly the inline unit tests of `src`).

use ace_corpus::*;

/// Every case is deterministic and exactly `bytes` long.
#[test]
fn cases_are_deterministic() {
    for case in FalsePositiveCase::ALL {
        let a = case.generate(10_001);
        assert_eq!(a.len(), 10_001, "{}", case.name());
        assert_eq!(a, case.generate(10_001), "{}", case.name());
    }
}

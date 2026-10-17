//! Tests of `ace_bitpack` `delta` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// Deltas round-trip and treat a lane wrap as a +1 step.
#[test]
fn delta_roundtrip_and_wrap() {
    let values: Vec<u16> = vec![65_533, 65_534, 65_535, 0, 1, 2];
    let deltas = delta(&values);
    assert_eq!(
        deltas,
        vec![1, 1, 1, 1, 1],
        "wrap must not create a 17-bit jump"
    );
    assert_eq!(undelta(values[0], &deltas), values);
    let big = vec![u64::MAX, 3, 1];
    assert_eq!(undelta(big[0], &delta(&big)), big);
}

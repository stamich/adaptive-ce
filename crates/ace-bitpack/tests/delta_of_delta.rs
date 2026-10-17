//! Tests of `ace_bitpack` `delta_of_delta` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// DoD round-trips for counters and is all-zero for fixed steps, including a lane wrap.
#[test]
fn dod_roundtrip() {
    let values = vec![1000u32, 1003, 1006, 1009, 1012, 1016];
    let (first, dod) = delta_of_delta(&values);
    assert_eq!(undelta_of_delta(values[0], first, &dod), values);

    let wrapping: Vec<u16> = vec![65_533, 65_534, 65_535, 0, 1, 2];
    let (first, dod) = delta_of_delta(&wrapping);
    assert_eq!(dod, vec![0, 0, 0, 0]);
    assert_eq!(undelta_of_delta(wrapping[0], first, &dod), wrapping);
    assert_eq!(delta_of_delta::<u64>(&[7]), (0, vec![]));
}

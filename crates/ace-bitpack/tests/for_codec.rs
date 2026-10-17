//! Tests of `ace_bitpack` `for_codec` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// FOR round-trips and stores offsets from the minimum.
#[test]
fn for_roundtrip() {
    let values = vec![1_000_000u32, 1_000_003, 1_000_002, 1_000_007];
    let (base, offsets) = frame_of_reference(&values);
    assert_eq!(base, 1_000_000);
    assert_eq!(offsets, vec![0, 3, 2, 7]);
    assert_eq!(unframe_of_reference(base, &offsets), values);
}

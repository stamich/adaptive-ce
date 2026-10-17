//! Tests of `ace_transforms` `delta` (formerly the inline unit tests of `src`).

use ace_transforms::*;

/// Exact round-trip including wrapping transitions; the first byte is stored verbatim.
#[test]
fn delta_round_trip() {
    let data = [254, 255, 0, 1, 200, 12];
    let encoded = delta_encode(&data);
    assert_eq!(encoded[..2], [254, 1]);
    assert_eq!(delta_decode(&encoded), data);
    assert!(delta_encode(&[]).is_empty());
}

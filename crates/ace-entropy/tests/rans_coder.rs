//! Tests of `ace_entropy` `rans_coder` (formerly the inline unit tests of `src`).

use ace_entropy::*;

/// Verifies exact rANS round-trip for skewed and full-alphabet inputs.
#[test]
fn round_trip() {
    let mut data = vec![b'a'; 20_000];
    data.extend((0u8..=255).cycle().take(10_000));
    let (metadata, payload) = rans_encode(&data).unwrap();
    let decoded = rans_decode(&metadata, &payload, data.len()).unwrap();
    assert_eq!(decoded, data);
}

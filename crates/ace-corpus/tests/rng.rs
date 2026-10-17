//! Tests of `ace_corpus` `rng` (formerly the inline unit tests of `src`).

use ace_corpus::*;

/// Sequences are fixed (golden values guard against accidental algorithm changes).
#[test]
fn sequences_are_stable() {
    let mut a = XorShift32::new(0x9E37_79B9);
    assert_eq!([a.next_u32(), a.next_u32()], [1_359_758_873, 3_761_132_862]);
    let mut b = XorShift64::new(1);
    assert_eq!(b.next_u64(), 0x4082_2041);
    assert_eq!(
        XorShift32::new(0).next_u32(),
        XorShift32::new(0x9E37_79B9).next_u32()
    );
}

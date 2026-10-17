//! Tests of `ace_corpus` `float_generators` (formerly the inline unit tests of `src`).

use ace_corpus::*;

/// First value of every float generator (guards the generators against accidental change).
#[test]
fn float_generators_are_stable() {
    let first = |data: Vec<u8>| {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&data[..8]);
        f64::from_le_bytes(raw)
    };
    assert_eq!(first(Workload::F64Constant.generate(8)), 21.5);
    assert_eq!(first(Workload::F64Step.generate(8)), 100.0);
    assert_eq!(first(Workload::F64Special.generate(8)).to_bits(), 0);
    assert!(Workload::F64Random
        .generate(1 << 16)
        .as_chunks::<8>()
        .0
        .iter()
        .all(|&c| (u64::from_le_bytes(c) >> 52) & 0x7FF != 0x7FF));
}

//! Tests of `ace_transforms` `dispatch` (formerly the inline unit tests of `src`).

use ace_core::TransformId;
use ace_transforms::*;

/// Every transform is inverted exactly by its inverse.
#[test]
fn dispatch_roundtrip() {
    let data = [254u8, 255, 0, 1, 200, 12];
    for id in [TransformId::None, TransformId::DeltaByte] {
        assert_eq!(
            invert_transform(id, &apply_transform(id, &data).unwrap()).unwrap(),
            data
        );
    }
}

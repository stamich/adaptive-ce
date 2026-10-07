//! `TransformId` → transform dispatch.

use ace_core::{AceResult, TransformId};

use crate::{delta_decode, delta_encode};

/// Applies one forward preprocessing transform.
pub fn apply_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_encode(input)),
    }
}

/// Applies one inverse preprocessing transform.
pub fn invert_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_decode(input)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

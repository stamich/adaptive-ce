//! `TransformId` → transform dispatch.

use ace_core::{AceResult, TransformId};

use crate::{delta_decode, delta_encode};

/// Applies one forward preprocessing transform.
///
/// # Errors
///
/// Infallible for every transform defined today; the `Result` is part of the contract so a
/// validating transform can be added without an API change.
pub fn apply_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_encode(input)),
    }
}

/// Applies one inverse preprocessing transform.
///
/// # Errors
///
/// Infallible for every transform defined today; the `Result` is part of the contract so a
/// validating inverse can be added without an API change.
pub fn invert_transform(id: TransformId, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        TransformId::None => Ok(input.to_vec()),
        TransformId::DeltaByte => Ok(delta_decode(input)),
    }
}

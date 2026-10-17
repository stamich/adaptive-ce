//! Frame-of-reference (FOR): values stored as offsets from their minimum.

use crate::Lane;

/// Returns `(min, v[i] - min)`; the base of an empty slice is zero.
pub fn frame_of_reference<T: Lane>(values: &[T]) -> (T, Vec<T>) {
    let base = values.iter().copied().min().unwrap_or_default();
    (
        base,
        values
            .iter()
            .map(|&value| value.wrapping_sub(base))
            .collect(),
    )
}

/// Reconstructs `base + offset` for every offset (inverse of [`frame_of_reference`]).
pub fn unframe_of_reference<T: Lane>(base: T, offsets: &[T]) -> Vec<T> {
    unframe_iter(base, offsets.iter().copied()).collect()
}

/// Lazy form of [`unframe_of_reference`].
pub fn unframe_iter<T: Lane>(base: T, offsets: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    offsets.map(move |offset| base.wrapping_add(offset))
}

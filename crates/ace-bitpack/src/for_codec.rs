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

#[cfg(test)]
mod tests {
    use super::*;

    /// FOR round-trips and stores offsets from the minimum.
    #[test]
    fn for_roundtrip() {
        let values = vec![1_000_000u32, 1_000_003, 1_000_002, 1_000_007];
        let (base, offsets) = frame_of_reference(&values);
        assert_eq!(base, 1_000_000);
        assert_eq!(offsets, vec![0, 3, 2, 7]);
        assert_eq!(unframe_of_reference(base, &offsets), values);
    }
}

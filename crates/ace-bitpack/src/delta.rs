//! First-order wrapping deltas.

use crate::Lane;

/// Computes `v[i+1] - v[i]` in the lane ring, reinterpreted as signed.
///
/// A wrapping 16-bit counter `65535, 0` therefore yields `+1`, not `-65535`.
pub fn delta<T: Lane>(values: &[T]) -> Vec<T::Signed> {
    values
        .windows(2)
        .map(|pair| pair[1].wrapping_sub(pair[0]).to_signed())
        .collect()
}

/// Reconstructs `first, first + d0, first + d0 + d1, …` (inverse of [`delta`]).
pub fn undelta<T: Lane>(first: T, deltas: &[T::Signed]) -> Vec<T> {
    undelta_iter(first, deltas.iter().copied()).collect()
}

/// Lazy form of [`undelta`]: yields `first` followed by the running wrapping sums.
pub fn undelta_iter<T: Lane>(
    first: T,
    deltas: impl Iterator<Item = T::Signed>,
) -> impl Iterator<Item = T> {
    std::iter::once(first).chain(deltas.scan(first, |current, step| {
        *current = current.wrapping_add(T::from_signed(step));
        Some(*current)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deltas round-trip and treat a lane wrap as a +1 step.
    #[test]
    fn delta_roundtrip_and_wrap() {
        let values: Vec<u16> = vec![65_533, 65_534, 65_535, 0, 1, 2];
        let deltas = delta(&values);
        assert_eq!(
            deltas,
            vec![1, 1, 1, 1, 1],
            "wrap must not create a 17-bit jump"
        );
        assert_eq!(undelta(values[0], &deltas), values);
        let big = vec![u64::MAX, 3, 1];
        assert_eq!(undelta(big[0], &delta(&big)), big);
    }
}
